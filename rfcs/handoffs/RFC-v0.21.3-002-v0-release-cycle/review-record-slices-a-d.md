# Review Record — RFC-v0.21.3-002, Slices A–D, and release recommendation

**Reviewer:** architect
**Commits reviewed:** `219e406` (A), `6039eaa` (B), `c8b1655` (C), `947191d` (D)
**Date:** 2026-07-30

## Outcome

**Approved.** All four slices accepted as executed. No corrections required —
the first submission in this release line with none.

This record also carries the **release recommendation** for `0.21.3` (§5), which
is the architect deliverable the owner needs before tagging.

## 1. Verified independently

| Claim | Method | Result |
|---|---|---|
| Cycle doc exists and is operative, not a stub | read `docs/src/release/v0-release-cycle.md` (122 lines) | **Confirmed** |
| Wired into mdBook | `SUMMARY.md:59` | **Confirmed** |
| Checklist repairs | read the `6039eaa` diff | **Confirmed** — `xtask docs build` removed with explanation, `git tag -s v1.0.0` → `1.0.0`, `xtask release --version` → `package-release` |
| IMP-06 corrected in both locations | grep both files | **Confirmed** — correction appended in `decision-log.md` and `implementation-notes.md` |
| Archive convention stated once | read `v0-release-cycle.md` §Release archive convention | **Confirmed** — see §2 |
| Link integrity | mechanical check of the cycle doc, release record, and checklist | **Confirmed** — 0 broken in all three |
| Gates green at **HEAD**, not just the evidence commit | re-ran myself at `947191d` | **Confirmed** — see §3 |
| Step 9 finding (`target/release-bundles/`) | `grep -rn release-bundles --include="*.rs"` | **Confirmed** — the path appears nowhere in the codebase |

## 2. The archive convention — exactly right

This was the item most likely to be got subtly wrong, because the obvious
phrasing ("we deviate from the rule because…") is precisely what the owner
rejected. What shipped:

> This is this project's convention, stated once, here — not an exception logged
> against a generic rule, and not duplicated elsewhere.

That is a single authoritative statement, with the reason attached, and no
cross-reference forcing a reader to hold two documents in mind. It reads as a
convention rather than an apology for one. Correct.

## 3. The evidence-commit gap — closed by re-running, not by reasoning

The record names `c8b1655` as its evidence commit while HEAD is `947191d`. That
is inherent — a release record cannot contain the output of a run that includes
itself — but it means the tagged tree is not literally the tree the evidence came
from, and that deserved checking rather than an argument.

I verified the delta is docs-only (`git diff --stat c8b1655 947191d`: the record
itself plus a date in one CHANGELOG heading), then re-ran the gates at HEAD:

```
cargo metadata --no-deps        exit 0
cargo fmt --all --check         exit 0
cargo xtask release-rehearsal   exit 0 — RELEASE-REHEARSAL: ALL MECHANICAL GATES PASS
                                Gate 10 MACHINE-CHECKED-PASS; Gate 9 [ -- ] MANUAL, unsigned
```

I did **not** re-run the 14-minute full QEMU sweep at HEAD. Stating that plainly
rather than implying I did: the delta between the two commits is two
documentation files, neither of which any QEMU tier reads, so criterion 5's
evidence transfers. If that judgement is wrong, the fix is one command.

## 4. Judgement calls — all accepted, two notable

- **Fixing IMP-06 in both files**, not just the one the handoff named. The
  handoff said "the decision log"; both files carry the identical misleading row.
  Fixing one would have left the other saying the same wrong thing. Correct, and
  the right instinct — the handoff's wording was mine and was too narrow.
- **Refusing to fabricate the current Verus release's GitHub asset URL.** They
  retitled the old hand-unpack recipe as a reference for the retired pin rather
  than invent a download path they could not verify. That is exactly the standard
  this release line exists to enforce, applied to their own work without being
  asked.
- **Dating the CHANGELOG heading** despite no prior entry having a date, because
  exit criterion 7 requires it, and *not* retroactively dating history. Right on
  both halves.
- **Re-checking `release_required` against `verus-targets.toml`** rather than
  trusting the tier history established earlier in this same review chain.
- **Not touching Steps 9–10** and flagging the bundle-signing gap instead. See
  §6.

## 5. Release recommendation — `0.21.3`

**Recommendation: release.**

| Item | Status |
|---|---|
| Version | `0.21.3` |
| Source commit | `947191d` |
| Entry criteria | 4/4 PASS |
| Exit criteria | 8/8 PASS, re-verified at HEAD for the mechanical set |
| Mechanical gates | 10/10 PASS |
| Gate 9 (manual, v1.0-scoped) | Unsigned — correctly out of scope for a v0 patch |
| Accepted-risk statement | **None required** |
| Supersedes | `0.21.2` (`KNOWN-BAD`) |
| Rollback | `0.21.2` is unbuildable; the practical predecessor is `0.21.1` |

Included: the RFC-v0.21.3-001 build restoration and as-built reconciliation, and
the RFC-v0.21.3-002 release cycle. Excluded: anything touching kernel behaviour,
the ABI surface, or v1.0 scope — this release adds no OS functionality and
changes no security boundary.

**Why this one is different.** Every previous "all gates pass" claim in this
project was made when the gates could not run, or was assembled from separate
partial submissions. This is the first release whose full table was produced from
a continuous evidence run and then independently re-verified by the reviewer. The
claim is now checkable, which is the property the project sells.

Remaining owner action: apply the tag. Per the roles table that is yours, not
mine and not the implementer's.

## 6. New finding — carried, not blocking

**v1.0 checklist Step 9 references a build output that does not exist.** Step 9
globs `target/release-bundles/*.bundle` to sign; the string `release-bundles`
appears nowhere in `crates/` or `tools/`. Confirmed by grep.

Two readings — checklist drift, or v1.0 infrastructure that was specified and
never built — and distinguishing them is a real investigation, not a lookup. The
implementer correctly declined to guess.

It does not block `0.21.3`: it is v1.0-specific, and Gate 9 is unsigned. But it
matters more than its size suggests, because Steps 9–10 are the *signing* steps.
If that infrastructure does not exist, the v1.0 checklist cannot be executed to
completion, and that is the third defect found in a document whose opening line
claims it should be run exactly. **Tracked as a v0.22 item**, alongside the
question of whether the whole v1.0 checklist needs an executability audit rather
than spot repairs.

## 7. RFC disposition

RFC-v0.21.3-002's acceptance criteria are met. It moves to `rfcs/done/` with
`Status: Implemented (v0.21.3)` **as part of the release commit series**, with the
reference sweep — same rule that applied to RFC-v0.21.3-001, including the
`docs/release/records/0.21.3.md` header link, which currently points at
`rfcs/proposed/`.

That sweep and the tag are the only things left.
