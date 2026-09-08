# RFC-0.29-001: 305 tests nothing runs, and five lists that disagree about one thing

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.29
**Tracks.** **E-013** and **E-015**, the 0.24 audit's two longest-deferred
findings — unscheduled for six milestones, and named as the cited root of seven
errata filed since.
**Touches.** `crates/fjell-tools` (`test_all`, `negative`),
`.github/workflows/ci.yml`. **Does not touch the kernel, the ABI, or any
service.**
**Relates to:** RFC-0.24-001 (which found both); RFC-0.28-005 (which closed one
E-015 instance and established the template); RFC-v0.22-001 (a new tier is a new
instrument and must be demonstrated failing).

## Summary

Two findings, one milestone, and they are entangled: **E-013's second half is an
E-015 instance.** The gate tools' tests do not run partly because `--lib` cannot
reach them and partly because CI never names them.

### E-013 — the number has nearly doubled since it was written

The erratum records **166** `#[test]` functions that `test-all` tier 1 cannot
reach. Re-derived today:

| | Count |
|---|---|
| Unit tests in crates with **no lib target** (`--lib` reaches none) | **285** |
| Tests in integration `tests/` dirs, excluding `fjell-proptest`'s 24 (tier 2 runs those) | **20** |
| **Total unreachable by tier 1** | **305** |

The composition is the point, and it is worse than in 0.24: the largest two are
**`fjell-consistency-check` (98)** and **`fjell-tools` (86)** — Gate 12's ten
subchecks and Gate 11's five callsite checks, plus `fjell-abi-snapshot` (37,
Gate 4), `fjell-unsafe-audit` (10, Gate 2), `fjell-mmio-audit` (7, Gate 3),
`fjell-readiness-check` (5, Gate 5).

**Everything 0.28 built to guard the tree is inside the blind spot.**
`SYSCALL-CALLSITE-001` and `-002` carry 36 of those tests. A regression in
either would be caught by nothing: the guard keeps reporting `PASS`, and the
only thing that checks the guard is a suite no gate invokes.

```
$ cargo test -p fjell-tools --lib
error: no library targets found in package `fjell-tools`
$ cargo test -p fjell-tools --bins
test result: ok. 86 passed
```

**The fix is one flag and one decision about which flag.** That is why this goes
first: it is the cheapest of the four audit findings and the only one that has
been actively worsening, because every instrument added since lands in it.

### E-015 — five lists, five answers, one subject

The negative-test categories are enumerated in five places:

| Where | Says |
|---|---|
| `test_all.rs` doc-comment | *"× 9 categories"* |
| `NEG_CATEGORIES` (what `test-all` actually runs) | **12** |
| `ci.yml`'s `ci-qemu-negative` matrix | **9** |
| `tests/qemu/profiles/*.toml` on disk | **15** |
| `KNOWN_V01X_CATEGORIES` + `KNOWN_V02_CATEGORIES` | 13 entries, including an alias |

No two agree. Concretely: **`semantic`, `uart-rx` and `uart-rx-unbound` run in
`test-all` and have never run in ordinary CI**; the `KNOWN_*` lists still name
`store`, `upgrade`, `lease` and `evidence`, none of which `NEG_CATEGORIES`
contains, and omit four that it does; and `test_all.rs`'s own doc-comment is
stale against the constant three lines below it.

Separately, **23 of 93 workspace crates are never named in `ci.yml`** — the
erratum recorded 21 of 91, and it went stale the same way it describes.

## The settled part

**D1 — Derive, do not enumerate.** RFC-0.28-005 established the template:
`git ls-files` answered "what is this repository" and two different queries
answered two different questions. The negative categories have an equivalent
authority on disk — `tests/qemu/profiles/*.toml` — and CI's crate coverage has
one in `cargo metadata`.

**D2 — A new tier is a new instrument** (RFC-v0.22-001). Whatever runs the 305
must be **demonstrated failing** on a deliberately broken test before it is
trusted. A tier that passes because it runs nothing is `ci-proptest`, which this
project has already shipped once.

**D3 — Do not reach for `--all-targets` without saying what it pulls in.**
`--bins` reaches the 285; `--tests` reaches the 20; `--all-targets` reaches both
plus benches and examples, some of which may want a target this host does not
have. **Say what each flag reaches and choose deliberately** — the original
defect is precisely that `--lib` was chosen without checking what it excluded.

**D4 — The stale doc-comment is a symptom, not the defect.** Correcting
*"× 9 categories"* to *"× 12"* would leave five lists that can disagree again
tomorrow. Fix the lists; the comment should then have nothing to be stale
against.

**D5 — `store` and `upgrade` are documented as having no emitting scenarios**
(`v1-limitations.md`). Deriving the CI matrix from the profiles on disk will
sweep them in. That is a **decision to make explicitly**, not a side effect to
discover when CI goes red.

## The open question — §6

**What is the authority for "the negative-test categories"?**

1. **The profiles on disk.** Everything derives from `tests/qemu/profiles/`.
   Simplest, and forces D5's decision immediately — a profile with no emitting
   scenario either runs and fails honestly, or is marked in the profile itself.
2. **One constant, and everything else derives from it.** `NEG_CATEGORIES`
   becomes the authority; CI reads it; a profile with no entry is an error.
   Keeps a hand-written list, but exactly one.
3. **The profiles, with an opt-out recorded in the profile.** A `.toml` key
   saying "not release-gated, and why" — derived scope with the exception
   stated where a reader will find it.

**Answer in writing before implementing.** I lean to 3, because D5's two
profiles are real and shape 1 forces them red while shape 2 keeps a list. It is
a lean.

## Requirements

**R1 — Tier 1 reaches the 305.** Per D3, with the flag choice argued.

**R2 — The tier demonstrated failing** on a deliberately broken test in a gate
tool — ideally one of the 36 behind `SYSCALL-CALLSITE-002`, since that is the
case the erratum is about.

**R3 — The five category lists become one derived answer**, per §6.

**R4 — CI's crate coverage derived**, not enumerated. 23 of 93 are unnamed;
some are deliberate (the drill crates back Gate 8 and run at rehearsal time —
`v1-limitations.md` says so). **Deliberate exclusions must be recorded as such**,
not left as absences.

**R5 — E-013 and E-015 → `CLOSED`**, register and `v1-limitations.md` in the
same commit. If an E-015 instance survives, it is **not closed** — say which and
why, and leave it open.

## Scope

`test_all.rs`, `negative.rs`, `ci.yml`, the two errata.

### Non-goals

- **E-014** — the literal-predicate family. It is the next milestone's subject
  and slicing it here is how it gets half-done.
- **E-017** — being checked separately; it may already be satisfied by
  RFC-v0.22-001.
- Making `fjell-kernel` host-testable. Its 30 tests are in the 285 and will
  start running; that is not the same as E-013's kernel half.
- Any kernel, ABI, or service change.

## Risks

**305 tests that have never run in CI will run for the first time.** Expect some
to fail — on this host, in CI, or both. **A failure is a finding, not an
obstacle**, and the instinct to exclude the crate that fails is the instinct
that produced `--lib`.

**CI time will grow**, and the negative matrix growing from 9 to 12 grows it
most. If that forces a scheduling change, say so; do not quietly drop a category
to keep the runtime down — that is E-031, which this project shipped once.
