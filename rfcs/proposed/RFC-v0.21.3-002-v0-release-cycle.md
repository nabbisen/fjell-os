# RFC-v0.21.3-002: v0 Development Release Cycle

**Status:** Proposed
**Milestone:** v0.21.3
**Tracks.** Cross-cutting release governance. Not tied to a feature.
**Touches.** `docs/release/release-checklist.md`, `CHANGELOG.md`, tag
conventions, `crates/fjell-tools/src/package_release.rs`.
**Relates to:** RFC-v0.15-003 (v1.0 release checklist), RFC 046 (v0.1.x
checklist), RFC 000 (lifecycle policy — precedent for policy-as-RFC)

## Summary

Fjell has cut **84 tagged releases**. It has a rigorous release checklist for
**one** of them — v1.0 — which has never been executed. The 84 that actually
happened ran without a defined cycle, and the most recent one shipped a tree
that does not build.

This RFC defines the *v0 development release cycle*: the lightweight,
actually-used path. It does not replace RFC-v0.15-003; it fills the gap
beneath it and repairs the drift found in it.

## Motivation

### M1 — v0.21.2 was tagged on a tree that does not build

```
tag 0.21.2 → 891a1ec → Cargo.toml:  members = [ "crates/*,     ← unterminated
```

Nothing in that release builds; no gate in it can run. The manifest defect is
already governed by RFC-v0.21.3-001. What *this* RFC addresses is separate and
narrower: the defect reached a **tag**. Committing a broken manifest is an
implementation error. Tagging it is a release-cycle failure.

### M2 — The cadence is batched, not cyclic

`0.19.0`, `0.20.0`, `0.20.1`, `0.20.2`, `0.21.0`, `0.21.1`, `0.21.2` — seven
releases, all tagged **2026-07-23**. The previous burst was 2026-06-07.

The project rules ask for regular releases at logical breaking points, with a
resolved RFC or a completed audit as the trigger. Seven tags in a day is a batch
dump. It is also *causally* connected to M1: with that cadence no gate run
occurred between tags, and the last three tags landed after the manifest broke,
when no gate run was even possible.

### M3 — The existing checklist is scoped to v1.0 and has drifted

`docs/release/release-checklist.md` is genuinely rigorous — pre-flight
conditions, gates, tagging at Step 11 *after* the gates. Two problems:

1. **Scope.** It opens *"Run this exactly to produce a v1.0 release."* It is a
   v1.0 procedure. RFC 046 defined a v0.1.x checklist. Everything between
   v0.2.0 and v0.21.2 — the overwhelming majority of this project's releases —
   is covered by neither.
2. **Drift.** Two of its twelve steps invoke subcommands that do not exist:

   | Step | Checklist says | Reality |
   |---|---|---|
   | 6 — Documentation build | `cargo xtask docs build` | no `docs` subcommand |
   | 12 — Package | `cargo xtask release --version v1.0.0` | subcommand is `package-release` |

   The checklist is not executable as written. This is consistent with it never
   having been run end to end.

### M4 — Tag and archive naming disagree

Tags are bare (`0.21.2`); archives are `fjell-os-v{version}.tar.gz`; the v1.0
checklist Step 11 says `git tag -s v1.0.0` — a *third* convention. Minor, but it
means no single string identifies a release across git, archive, and docs.

### M5 — An unrecorded deviation from the project rules

The project rules state that a release archive must unpack its files to the
archive root, and explicitly mark `/project-v1.0.0/file1` as ❌ Bad.
`package_release.rs:40` sets `internal_dir = fjell-os-v{version}`, and decision
IMP-06 records this as settled — with no note that it contradicts the rule.

The governing organisation document requires that a project-specific deviation
from the baseline rules be *explicitly documented and approved by the owner*. No
such record exists. The deviation may well be the right choice; the defect is
that it is undocumented either way. See §7 (Decision request 1).

## Goals

1. Define entry criteria, exit criteria, and required artifacts for a v0
   development release.
2. Make a tag a claim backed by re-derivable evidence.
3. Set a cadence rule that matches the project's stated trigger.
4. Repair the drift in the v1.0 checklist rather than writing a competing one.
5. Define how a known-bad release is recorded.

## Non-goals

- Replacing RFC-v0.15-003. The v1.0 checklist stays authoritative for v1.0.
- Adding CI enforcement. This is a documented cycle, not new automation;
  automation may follow once the cycle has been used a few times.
- Changing version-numbering semantics.
- Deciding v1.0 timing or scope — owner authority, untouched.

## The v0 development release cycle

### When to cut (trigger)

Cut a release at a **logical breaking point**, per the project rules:

- an RFC reaches a disposition, or
- a major theme inside a large RFC is completed, or
- a compliance process (doc review, audit) completes.

**Do not batch.** If two triggers occur close together, cut two releases. A tag
is cheap; an unverifiable tag is expensive.

### Entry criteria

A release cycle may begin when:

1. Working tree is clean (`git status --short` empty).
2. The governing RFC's slices are complete and architect-reviewed.
3. No review finding is open at "corrections required".
4. The version in `Cargo.toml` is the version being released.

### Exit criteria — all required before the tag

| # | Criterion | Evidence |
|---|---|---|
| 1 | Workspace resolves | `cargo metadata --no-deps` exit 0 |
| 2 | Build clean | `cargo xtask build`, warning count recorded |
| 3 | Formatting | `cargo fmt --all --check` exit 0 |
| 4 | Host tiers | `cargo xtask test-all --no-qemu` |
| 5 | QEMU tiers | `cargo xtask test-all` |
| 6 | Mechanical gates | `cargo xtask release-rehearsal`, full gate table recorded |
| 7 | CHANGELOG entry | present, version and date correct |
| 8 | Docs match reality | no doc asserts behaviour the tree does not have |

Criterion 1 is listed first and separately on purpose: v0.21.2 failed *only*
that one, and failing it makes every other criterion unevaluable.

**A gate that cannot run is not a passing gate.** Gate 10 reporting
`CONFORMANCE-ONLY` because Verus is absent is a **failure**, not an
abstention — this is existing policy (RFC-v0.18-001) and is restated here
because it is the criterion most likely to be softened under time pressure.

### Required artifacts per release

1. A git tag.
2. A CHANGELOG entry.
3. **A release record** committed at `docs/release/records/<version>.md`
   containing the exit-criteria table with real command output, the gate table,
   known limitations, and any accepted-risk statement.

The release record is the substantive change this RFC introduces. Today, "all
eleven gates pass" lives in prose in a handoff bundle and cannot be
re-derived after the fact. A tag without a record is a claim without evidence —
which is precisely the failure mode this project exists to eliminate in its
*product*, and should not tolerate in its *process*.

### Roles

| Step | Owner | Architect | Implementer |
|---|---|---|---|
| Decide a release is warranted | A | R | I |
| Verify exit criteria | I | A | R |
| Produce the release record | I | C | R |
| Apply the tag | A | C | R |
| Approve v1.0.0 specifically | **A/R** | C | I |

v1.0.0 remains under explicit owner publication control (DEC-002), unchanged.

### Known-bad releases

When a released version is later found unusable, add a **`KNOWN-BAD`** note to
its CHANGELOG entry naming the defect and the superseding version. Do not delete
or move the tag — history stays honest; the record explains it.

Applies immediately to `0.21.2` (see §7, Decision request 2).

## Repairs to the existing checklist

Within this RFC's scope, in `docs/release/release-checklist.md`:

1. Step 6: `cargo xtask docs build` → a command that exists, or remove the step
   and state that docs are built by mdBook directly.
2. Step 12: `cargo xtask release --version` → `cargo xtask package-release`.
3. Retitle to make its v1.0 scope explicit, and cross-link this RFC for v0.
4. Reconcile Step 11's `v1.0.0` tag string with the bare-tag convention (§M4).

## Alternatives considered

| Option | Assessment |
|---|---|
| **Define the v0 cycle; repair the v1.0 checklist** *(chosen)* | Fills the real gap; avoids a second competing policy — the anti-pattern RFC 000 warns about. |
| Extend RFC-v0.15-003 to cover v0 | The v1.0 procedure (bundle signing, offline release key, attestation) is far too heavy for a patch release. Applied to v0 it would be routed around, which is how processes die. |
| CI-enforce everything now | Enforcement before the cycle has been used is premature; it also cannot enforce criterion 8 (docs match reality), which needs judgement. |
| Do nothing; treat v0.21.2 as a one-off | Rejected. It was not caused by bad luck but by an absent exit criterion, and the cadence that produced it is unchanged. |

## Acceptance criteria

- [ ] This cycle is documented at `docs/src/release/v0-release-cycle.md` and
      linked from `SUMMARY.md`.
- [ ] `docs/release/release-checklist.md` steps 6 and 12 invoke real
      subcommands; scope-titled for v1.0; cross-links this RFC.
- [ ] `docs/release/records/` exists with a record for the first release cut
      under this cycle.
- [ ] `CHANGELOG.md` marks `0.21.2` per Decision request 2.
- [ ] Tag/archive naming is consistent, or the difference is documented.
- [ ] The archive-layout deviation is recorded per Decision request 1.

## Risks

| ID | Risk | Mitigation |
|---|---|---|
| R1 | The cycle is documented and then ignored, as the v1.0 checklist was | The release record makes skipping visible: no record, no evidence, and the omission is in git |
| R2 | Per-trigger cadence produces many small releases | That is the intent. Cheap tags with evidence beat rare tags without |
| R3 | Exit criterion 8 (docs match reality) is subjective | It is. It is also exactly the class of defect RFC-v0.21.3-001 found, so it stays — with architect review as the check |

## Decision requests (owner)

### Decision request 1 — release archive layout

**Decision required.** Archives currently unpack to `fjell-os-v{version}/`.
Your project rules mark an intermediate parent directory as ❌ Bad.

| Option | Benefit | Drawback |
|---|---|---|
| **(A) Keep the parent dir; record as an approved deviation** *(recommended)* | Safe extraction — cannot scatter files into the user's cwd; matches common convention and IMP-06 as built | Contradicts the rule as written; needs an explicit approval record |
| (B) Change `package_release.rs` to match the rule | Rules and practice agree with no exception | Extracting scatters files into the current directory; reverses a shipped convention |

Recommendation: **(A)**. The rule reads as a general default; this project has a
specific reason. But it is your rule, so the deviation needs your approval
rather than my inference. Consequence of deferring: IMP-06 remains an
undocumented rule violation.

### Decision request 2 — disposition of `0.21.2`

| Option | Benefit | Drawback |
|---|---|---|
| **(A) Mark `KNOWN-BAD` in the CHANGELOG** *(recommended)* | Honest history; anyone landing on that tag learns why it fails | Publicly records a bad release |
| (B) Say nothing; let `0.21.3` supersede it | Tidier history | Someone will check out `0.21.2`, hit an unbuildable tree, and have nothing to tell them it is known |

Recommendation: **(A)**. A project whose value proposition is checkable claims
should not have a silent unbuildable tag in its history.

## Blast radius of M1 — checked, not assumed

The other six tags from the 2026-07-23 batch were checked against exit
criterion 1:

| Tag | Manifest |
|---|---|
| `0.19.0`, `0.20.0`, `0.20.1`, `0.20.2`, `0.21.0`, `0.21.1` | OK |
| `0.21.2` | **BROKEN** |

So the damage is confined to the final tag of the batch. `0.21.2` is the only
known-bad release, which is what Decision request 2 acts on. This does not
clear the other six against criteria 2–8 — only criterion 1 was checked — but
it does bound the problem.
