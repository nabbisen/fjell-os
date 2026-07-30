# v0 Development Release Cycle

*Governed by RFC-v0.21.3-002. This is the operative procedure — follow it,
don't just read it once. For a v1.0 release, use
[the v1.0 release checklist](../../release/release-checklist.md)
instead; this cycle does not apply there.*

This is the lightweight, actually-used release path for every v0.x release.
It does not replace the v1.0 checklist — it fills the gap beneath it. Every
release before v1.0.0 goes through this cycle.

## When to cut a release (trigger)

Cut a release at a **logical breaking point**:

- an RFC reaches a disposition, or
- a major theme inside a large RFC is completed, or
- a compliance process (a doc review, an audit) completes.

**Do not batch releases.** If two triggers occur close together, cut two
releases, not one combined tag. A tag is cheap to make; an unverifiable tag
is expensive to live with.

## Entry criteria — before starting the cycle

All four must hold before beginning:

1. Working tree is clean (`git status --short` is empty).
2. The governing RFC's slices are complete and architect-reviewed.
3. No review finding is open at "corrections required".
4. The version in `Cargo.toml` is the version being released.

## Exit criteria — all required before the tag

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

**Criterion 1 is listed first and separately on purpose.** `0.21.2` failed
only this one criterion, and failing it makes every other criterion
unevaluable — a workspace that doesn't resolve means no gate, no test, and
no build can even run to tell you whether it would otherwise have passed.

**A gate that cannot run is not a passing gate.** If Gate 10 (Verus) reports
`CONFORMANCE-ONLY` because the prover is absent, that is a **failure**, not
an abstention (RFC-v0.18-001). This is the criterion most likely to be
softened under time pressure — it must not be.

## Required artifacts per release

1. **A git tag** — bare version, no `v` prefix (e.g. `0.21.3`, not `v0.21.3`;
   Rust crate convention).
2. **A CHANGELOG entry**, dated, under the version being released.
3. **A release record**, committed at `docs/release/records/<version>.md`,
   containing:
   - the exit-criteria table above, with real command output for each row
   - the full `release-rehearsal` gate table
   - known limitations at this release
   - an accepted-risk statement, if and only if a gate that should block did
     not pass and the owner explicitly accepted the risk — never written
     unilaterally to paper over a red gate

The release record is the substantive addition this cycle makes. Before it,
"all eleven gates pass" lived only in prose in a handoff bundle and could
not be re-derived after the fact. A tag without a record is a claim without
evidence — the exact failure mode this project exists to eliminate in its
product, and it should not be tolerated in its process either.

## Roles

| Step | Owner | Architect | Implementer |
|---|---|---|---|
| Decide a release is warranted | A | R | I |
| Verify exit criteria | I | A | R |
| Produce the release record | I | C | R |
| Apply the tag | A | C | R |
| Approve v1.0.0 specifically | **A/R** | C | I |

`v1.0.0` remains under explicit owner publication control (DEC-002),
unaffected by this cycle: the implementer prepares everything and stops:
the owner alone applies that specific tag, cycle or no cycle.

## Known-bad releases

When a released version is later found unusable, add a **`KNOWN-BAD`** note
to its `CHANGELOG.md` entry, naming the defect and the superseding version.
**Do not delete or move the tag** — history stays honest; the record
explains what went wrong rather than hiding that it happened.

`0.21.2` is marked `KNOWN-BAD` under this rule (RFC-v0.21.3-001, RFC-v0.21.3-002
Decision request 2): its workspace manifest did not parse, so nothing in
that release builds and no gate in it could run.

## Release archive convention

Fjell's release archive is `fjell-os-v{version}.tar.gz` and unpacks to a
single top-level `fjell-os-v{version}/` directory (e.g.
`fjell-os-v0.21.3.tar.gz` → `fjell-os-v0.21.3/README.md`, not `README.md`
directly at archive root). This is this project's convention, stated once,
here — not an exception logged against a generic rule, and not duplicated
elsewhere. The reason: extracting ~90 entries directly into the caller's
current directory has no undo; a single named parent directory does.
`crates/fjell-tools/src/package_release.rs` implements this and does not
change (RFC-v0.21.3-002 Decision request 1, owner-accepted 2026-07-30).

## What this cycle does not cover

- It does not replace [the v1.0 release checklist](../../release/release-checklist.md),
  which stays authoritative for `v1.0.0` specifically (bundle signing,
  offline release key, attestation — deliberately heavier than any v0
  release needs).
- It adds no CI enforcement. This is a documented, followed-by-hand cycle;
  automation may follow once it has been used a few times.
- It does not change version-numbering semantics or decide `v1.0` timing —
  both remain owner authority, untouched.
