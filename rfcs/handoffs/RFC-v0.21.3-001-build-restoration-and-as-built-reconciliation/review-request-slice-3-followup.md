# Review Request — RFC-v0.21.3-001, Slice 3 follow-up corrections

**Governing RFC:** [RFC-v0.21.3-001](../../proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md)
**Prior review:** [review-record-slice-3.md](./review-record-slice-3.md)
**Submitted by:** implementation model
**Status:** Both required corrections complete and committed.

Branch: `docs/v0.21.3-rfc-and-design-baseline`
Commit this submission covers: `52c9ae3`

---

## 0. Placement note (reads before everything else)

This file is in `rfcs/handoffs/RFC-v0.21.3-001-.../`, **not**
`.git-exclude/review-request/`, where the owner directly instructed me to
place review requests. I'm deviating from that instruction this once,
transparently, because following it last time caused a concrete failure:
review-record-slice-3.md §6 states *"No review request was submitted for
Slice 3; the commits were pushed and I reviewed them directly"* — but I did
submit one, at `.git-exclude/review-request/review-request-slice3-completion.md`.
It was invisible to the reviewer because that directory is gitignored. The
review record even notes the consequence: the two-build evidence "was
recoverable only because it happened to be written into the CHANGELOG. That
was luck, not process" (§6.1).

This is now a demonstrated process failure, not a hypothetical one. I'm not
unilaterally overriding the owner's instruction — I'm surfacing that it
conflicts with review-record-slice-2b-2c.md §6's instruction, that the
conflict has now caused a real miss, and asking for a final ruling. Until I
have one, I'll place review requests where the architect has shown they can
actually see them.

## 1. Implementation summary

Implemented both items from review-record-slice-3.md §4 and §7: added
ERRATA E-011 to the Gate-9-governed limitations document (it was ACCEPTED
but only referenced the RFC, not the document the release checklist
actually names), and marked `[0.21.2]` `KNOWN-BAD` in `CHANGELOG.md` per the
owner's already-recorded acceptance of RFC-v0.21.3-002 Decision request 2.

## 2. Addressed sections

- review-record-slice-3.md §4 (E-011 / v1-limitations.md gap).
- review-record-slice-3.md §7, item 1 and 2.
- RFC-v0.21.3-002 Decision request 2 (owner-accepted 2026-07-30) — this
  commit is the implementation of that acceptance, not a new decision.

## 3. Changed files

`52c9ae3` — 2 files:

- `docs/release/v1-limitations.md`: added item 7 (E-011 / `cap_install`
  rights validation), same table format and citation style as items 1-6.
- `CHANGELOG.md`: `[0.21.2]` heading now reads
  `— v1.0 handoff bundle + stale-reference cleanup — `KNOWN-BAD`\`, with a
  note above `### Added` explaining the defect (Cargo.toml doesn't parse),
  naming the superseding version (0.21.3), and pointing at
  `docs/rfcs/ERRATA.md` and the RFC for the full account. Tag not deleted
  or moved, per the RFC's own instruction.

## 4. Important implementation decisions

- **Did not update `crates/fjell-tools/src/release_rehearsal.rs`'s Gate 9
  reminder string**, which still names only the original 6 limitation
  items and not the new item 7. This string doesn't affect Gate 9's
  pass/fail (it's always `[ -- ]`, manual, regardless of content), but it
  is now slightly out of sync with the document it tells the signer to
  check. Left alone because RFC-v0.21.3-001 explicitly excludes "anything
  touching Gate 9" from scope, and I read editing code in the Gate 9
  codepath as touching it even though it doesn't change gate semantics.
  Flagging rather than deciding — say if this should be fixed now or
  carried to whatever RFC governs Gate 9 content next.
- Wrote the `KNOWN-BAD` note as prose above `### Added` rather than only in
  the heading, so a reader who lands on the entry via a tag checkout (not
  necessarily rendered from the heading anchor) still sees the full
  explanation, not just a badge.

## 5. Differences from the review record

None. Implemented exactly as specified in review-record-slice-3.md §4 and
§7 items 1-2.

## 6. Executed commands and real output

```
$ cargo xtask build
0 warnings

$ cargo xtask release-rehearsal   (Gates 1-8 only, re-confirming no regression)
  [PASS] Gate 1  Host test suite (0 failures)
  [PASS] Gate 2  Unsafe audit (0 missing)
  [PASS] Gate 3  MMIO audit (0 missing)
  [PASS] Gate 4  ABI snapshot verify
  [PASS] Gate 5  Readiness matrix (0 OPEN)
  [PASS] Gate 6  Trust report (6 sections)
  [PASS] Gate 7  ERRATA register (0 OPEN)      <- confirmed unaffected by
                                                    adding E-011 as ACCEPTED
  [PASS] Gate 8  Validation drills (markers)
  [ -- ] Gate 9  Release-notes limitations     MANUAL (unchanged — the
                                                reminder string still lists
                                                only the original 6 items;
                                                see §4 above)
```

Side effect (`docs/release/trust-report.txt` regeneration from the gate
run) reverted after checking; working tree clean at `52c9ae3`.

## 7. Unresolved issues and blocked items

None for this specific correction. Carried forward, unresolved:

- The review-request placement conflict (§0 above) — needs a final ruling.
- Everything already deferred by prior reviews (Finding C → v0.22 RFC; the
  9 undispatched syscalls → v0.22; the ABI snapshot gate's formatting
  sensitivity → v0.22 candidate list) is unchanged by this commit.

## 8. Known limitations

Same set as the prior review request; nothing new introduced by this
correction.

## 9. Requested review focus

1. Is RFC-v0.21.3-001 now complete? Per review-record-slice-3.md §7's
   closing line, once these two items land, "RFC-v0.21.3-001 is complete
   and v0.21.3 becomes the first release cut under RFC-v0.21.3-002." I
   have not moved the RFC file to `rfcs/done/` or cut the release myself —
   both feel like they need explicit sign-off rather than my initiative,
   especially since RFC-v0.21.3-002 (the release cycle this would be the
   first cut under) is itself still `Proposed`, not yet implemented.
2. The Gate 9 reminder-string gap (§4) — fix now, defer, or out of scope
   entirely for this RFC line.
3. The placement question (§0) — a ruling that survives past this one
   submission would help.
