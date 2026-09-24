# Developer Handoff — RFC-0.33-004

**Governing RFC:** [RFC-0.33-004](../../accepted/RFC-0.33-004-checks-that-cover-what-they-claim.md)
**Milestone:** 0.33
**Status:** inherited from the governing RFC (Accepted, 2026-09-24)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Four instruments, one shape

Each reports success over a smaller set than its name implies, and none says so.
**Nothing in this line is finished when the instrument is fixed; it is finished
when the instrument has been seen refusing the case it used to pass** (D7). That
is the whole difference between this line and the four errata it closes.

## 0.1 The finding that reproduced itself while the RFC waited

Finding 1 counted 85 `-p fjell-…` entries. At acceptance there were **101**
occurrences across 88 lines: the 0.34 line added two services and extended the
lists by hand, because nothing refuses a hand-written package list. **Quote
neither number.** R1 re-derives it at your tip with the original probe, and the
evidence carries that figure. The growth is the argument for D2, not a statistic.

## 0.2 Settled — do not re-open

1. CI runs **workspace-derived** invocations from **one shared definition** used
   by Gate 1, `test-all` and CI (D1).
2. A subcheck **refuses a hand-written package list in a test job**, demonstrated
   failing (D2).
3. `fjell-ci-coverage` and `[workspace.metadata.fjell.ci_excluded]` are **deleted**
   (D3).
4. `standards-mapping` and `evidence` accept an **absolute repository URL**; a
   citation resolving nowhere is still refused (D4).
5. The ABI item hash **covers an enum's variants**; the baseline is re-recorded
   **once**, with the invisible drift **named** (D5).
6. The profile reader **respects quoting**; a marker it cannot carry is **refused
   at load**, and `health-fail.toml`'s workaround comes out (D6).
7. Five demonstrations (D7).

## 1. Order — §E's lean, and why it is not negotiable here

**E-057 → E-056 → E-052 → E-049.** Smallest and most independent first, so that a
red gate is always attributable to the change that caused it. E-049 last because
it is the one that touches every test job: a red CI run during E-049's work should
never be ambiguous between "the derived invocation is wrong" and "something
earlier in this line broke".

Answer **§A–§E in writing before the first code change**, not per erratum.

## 2. E-056 is the dangerous one — §C

**A baseline regenerated without reading it is the instrument this project has
twice found reporting on nothing.** When the hash starts covering variants, the
re-record will reveal drift that accumulated invisibly. The rule:

- **Name every change in the commit message**, grouped: variants added (expected),
  variants **removed** (a finding — say when and by which commit), signature
  changes.
- A removed variant nobody noticed is escalated here, not absorbed.
- Show `--verify`'s output **before** regenerating, then after. The before-figure
  is the evidence; the after-figure is bookkeeping.

## 3. E-049 — what §A must measure, and the trap in it

The RFC's measured replacement is
`cargo test --workspace --lib --exclude fjell-proptest --features
fjell-sxt-crypto/crypto-profile-development` — 49 crates, 580 tests, exit 0.

- **Passing the feature is not optional.** `cargo test -p fjell-sxt-crypto --lib`
  alone fails its own guard today; a workspace run passes only because another
  crate enables the feature. Say in writing what feature unification hides, and
  whether any crate's own guard is now untested (that is the Risks section's
  question, and it needs an answer, not a mention).
- **Measure the wall-clock cost** of replacing the three `-p` jobs and state it.
  If replacing loses the v0.7-formats job's feature isolation, keep that job and
  say so — the goal is no crate untested, not fewer jobs.
- **One definition, three consumers.** If Gate 1, `test-all` and CI can still
  disagree after this line, D1 is not done.

## 4. D2 — what it forbids, and the message it prints

§B's lean is **test jobs only**: `ci-check` legitimately names packages for
`cargo check`. Whatever you decide, **the subcheck's own message states the rule**
— a reader who trips it must learn why from the failure, not from this handoff.
Demonstrate it by reintroducing a `-p` in a test job and showing the refusal.

## 5. E-052 — the site is the test

Converting citations to URLs turns `standards-mapping` and `evidence` red today,
because both resolve a citation as a filesystem path. Change those subchecks here
(§D) and **say what their tests now assert**. The evidence is not "the links look
right": it is **the published site**, with the converted citations followed from
it, in RFC-0.32-003 R8's shape.

## 6. Prohibited

- Quoting 85 or 101 as the entry count (§0.1).
- Regenerating the ABI baseline without reading the drift (§2).
- Dropping `--features` to make a crate's guard pass.
- Adding tests to crates that have none, or making the kernel host-testable
  (E-013 stays open).
- Changing what the compliance mapping **claims** — only how it cites.
- Rewriting the profile loader as a real TOML parser if quoting is enough.
- Leaving `fjell-ci-coverage` in the tree "until later".
- A gate piped into `grep` instead of its own exit status.

## 7. Required evidence

1. R1's four re-derivations, each with a control, at your tip.
2. §A–§E in writing, §A with the wall-clock figure and the feature-unification
   answer.
3. D1/D2/D3 — the derived invocations, the subcheck with its message, and the
   deletions.
4. D4 — the subchecks' new rule, the converted citations, and **the site
   checked**.
5. D5 — `--verify` before and after, with every revealed change named.
6. D6 — the loader's refusal, and `health-fail.toml` without its workaround.
7. R7's counts: tests CI now runs that it did not, citations resolving from the
   site, what the re-record revealed.
8. **E-049, E-052, E-056, E-057** resolved or survivors named; register and
   `v1-limitations.md` in the same commit.
9. The gates, each by **its own exit status**, `test-all`, and a CI run id.

## 8. Review request

Standard format, in `.git-exclude/review-request/`. Flag for focused review:

- **What the ABI re-record revealed**, first, and any removed variant.
- **The feature-unification answer**, and any crate guard now untested.
- **D2's message**, and the demonstration of it refusing a reintroduced list.
- The site check for the converted citations.
- Anything you had to change outside the *Touches* list, which is indicative.
