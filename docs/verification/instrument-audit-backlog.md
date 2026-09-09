# The Instrument Audit's Undisposed Findings — a decision brief

**Author:** architect
**Date:** 2026-09-08, after the `0.28.0` cut
**For:** the owner, setting the next milestone's theme
**Source:** [RFC-0.24-001](../../rfcs/done/RFC-0.24-001-instrument-audit.md) and
its [close-out](./instrument-audit-closeout.md)

---

## What this is

RFC-0.24-001 audited 58 instruments and produced 33 findings. Most were repaired
in 0.24. Four were dispositioned as errata and deferred: **E-013, E-014, E-015,
E-017**. They have been `unscheduled` ever since — **six milestones**, 0.24
through 0.28.

This is not an argument that deferring them was wrong. It is the evidence that
has accumulated since, put in one place so the decision can be made on it.

## The four

| | | Since |
|---|---|---|
| **E-013** | Gate tools' own tests are run by no mechanism — `--lib` cannot reach them and CI never names them | unchanged |
| **E-014** | Instruments that decide a semantic property by matching a fixed literal | unchanged |
| **E-015** | Instruments whose scope is a hand-written list that has drifted | one instance closed (E-025) |
| **E-017** | `sound` verdicts not all demonstration-backed | superseded in practice by RFC-v0.22-001, never formally closed |

## The evidence that accumulated

**Seven errata filed since name E-014 or E-015 as their family.** Not as a
passing resemblance — as the cited root:

| Erratum | Family | What it was |
|---|---|---|
| **E-013** | E-015 | the gate tools are absent from every enumerated CI list |
| **E-019** | E-014 | a negative profile passing on an unguaranteed ordering |
| **E-025** | E-015 | three tools, three disagreeing hand-written exclusion lists |
| **E-027** | E-014 | a "threat-model gate" asserted in documentation, never built |
| **E-031** | E-014 | an expectation file narrowed until it matched the defect |
| **E-037** | E-015 | the toolchain declared in two places that cannot see each other |
| **E-038** | both | three keeper files for one property of git, each found separately |

**The rate is increasing, not decaying.** One in 0.24, none in 0.25, one in
0.26, two in 0.27, **three in 0.28**. Each was closed as a *case*. None closed
the *class*.

That is the actual cost of the deferral: not that the four sit unfixed, but that
the project keeps paying to rediscover them one instance at a time, and the
per-instance discovery has been getting more expensive — E-031 and E-038 were
each found only because something else failed first.

## E-013 is the one that has quietly got worse

The other three are steady. E-013 has been deteriorating because **every
instrument added since lands inside it.**

`crates/fjell-tools` has `[[bin]]` and no `[lib]`:

```
$ cargo test -p fjell-tools --lib
error: no library targets found in package `fjell-tools`
```

Tier 1 is `cargo test --workspace --lib`. So it reaches none of them.

**Everything 0.28 built to guard the tree is in that blind spot.**
`SYSCALL-CALLSITE-001` and `-002` — the guards that stop a thirty-sixth
hand-rolled syscall block appearing, and that check the wrapper crate's own
register contracts — carry **36 unit tests in `fjell-tools`**. Gate 12's ten
subchecks carry their own in `fjell-consistency-check`. **None of them runs in
`test-all` or `release-rehearsal`.**

A regression in `SYSCALL-CALLSITE-002` would be caught by nothing. The guard
would keep reporting `PASS`, because the only thing that checks the guard is a
test suite no gate invokes.

E-013's own text records the composition: **166 `#[test]` functions across 10
crates, 8 of them the gate tools**, backing Gates 2, 3, 4, 5, 11 and 12. That
count predates 0.28 and is now larger.

## What each would cost

- **E-013 — small, and mostly mechanical.** Add a tier that runs the tests
  `--lib` cannot reach. The subtlety is *which* invocation, not whether: `--bins`
  and `--tests` reach different sets, and picking one without checking is how
  the original defect happened. Then name the gate tools in CI.
- **E-015 — a template already exists.** RFC-0.28-005 closed one instance by
  deriving scope from `git` rather than listing it, with two different queries
  for two different questions. The remaining instances (CI job lists, the
  negative-category matrix, `KNOWN_*_CATEGORIES`) are the same shape.
- **E-014 — the largest and the least mechanical.** "Decide by parsing, not by
  matching" has to be answered per instrument, and some of the literals are load
  bearing. This is a milestone, not a slice.
- **E-017 — counted 2026-09-09, and my guess below was wrong.** ~~probably
  already closed~~ It is **not** satisfied by RFC-v0.22-001: that made
  "demonstrated failing" a standing requirement *going forward*, which does not
  retroactively demonstrate the 0.24 audit's rows. But it is much smaller than
  "twenty assumed": of 21 `sound` rows, **8 have a first-hand demonstration**,
  **4 cite the tool's own unit suite** (the exact mode-2 defect this erratum
  names, uncorrected for Gate 2, Gate 11 and Gate 12 while Gate 4 was
  re-derived), **2 inherit another row's**, and **4 have none at all**. Real
  demonstrations now exist for most of the middle group — from RFC-0.24-002/003,
  0.27-001/003/004 and 0.28-002/004 — so much of the work is **correcting the
  register to cite evidence that already exists**, not producing new evidence.
  Full count in E-017's own entry. **A slice, not a milestone.**

## Recommendation

**Lead 0.29 with E-013, and take E-015 alongside it.**

E-013 first because it is the cheapest of the four, because its consequence is
the largest — five gates' own demonstrations are unexercised — and because it is
the only one actively worsening. Every guard this project adds makes the gap
bigger, and 0.28 added three.

E-015 alongside because the template exists and the instances are known.

**E-014 should be its own milestone**, not a slice of this one. It is the most
recurrent family and the least mechanical, and slicing it is how it would get
half-done.

**E-017 should be checked before it is scheduled.** If RFC-v0.22-001 already
satisfies it, closing it is a paragraph, and one of the four disappears without
a line.

## The decision requested

1. Whether 0.29's theme is the audit backlog at all.
2. If so: E-013 + E-015 together, as recommended, or a different pairing.
3. Whether E-017 is checked-and-closed first, which may shrink the set to three.

None of this changes what ships. All four are `ACCEPTED` and disclosed; Gate 7
is green; nothing here is a live drift. The question is only whether the project
keeps paying per instance.
