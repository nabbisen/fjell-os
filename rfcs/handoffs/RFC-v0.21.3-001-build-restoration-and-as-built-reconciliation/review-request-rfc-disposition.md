# Review Request — RFC-v0.21.3-001 disposition

**Governing RFC:** [rfcs/done/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md](../../rfcs/done/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md)
**Prior review:** [review-record-slice-3-completion.md](../../rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/review-record-slice-3-completion.md) §5
**Submitted by:** implementation model
**Status:** Both required items complete and committed in a single commit,
as directed.

Branch: `docs/v0.21.3-rfc-and-design-baseline`
Commit: `0a2bcfc`

---

## 1. Implementation summary

Completed both items review-record-slice-3-completion.md §5 required before
disposition: corrected the 3 remaining stale "Gate 9 is the only blocker"
claims in the frozen `handoff-0.21.2/` bundle, and moved the RFC to
`rfcs/done/` with a full reference sweep, re-filing `rfcs/README.md`
accordingly. One commit, as instructed.

Note on process: mid-task I attempted a broad sweep across all 11 files
containing the old `proposed/` path, including the historical
`review-request-*`/`review-record-*` documents under `rfcs/handoffs/`. That
was stopped before it ran. The correct scope — sweep exactly the 6 live
documents, leave the 5 dated review submissions alone — was relayed back to
me and is what actually shipped. Recording this so the distinction (live
docs vs. point-in-time submissions) is visible in the history, not just in
my working state.

## 2. Addressed sections

review-record-slice-3-completion.md §5, items 1 and 2, in full.

## 3. Changed files — commit `0a2bcfc` (10 files)

- `docs/src/releases/handoff-0.21.2/{README,testing-and-gates,project-summary}.md`
  — each gets a `*(Superseded — RFC-v0.21.3-001: ...)*` note in place,
  original v0.21.2-era claim left intact above it.
- `rfcs/proposed/RFC-v0.21.3-001-....md` → `rfcs/done/RFC-v0.21.3-001-....md`
  (git rename), `Status:` → `Implemented (v0.21.3)`.
- Reference sweep (6 files): `ROADMAP.md`, `CHANGELOG.md`,
  `docs/src/roadmap/roadmap.md`, `docs/src/external-design/kernel.md`,
  `rfcs/README.md`, and the RFC's own `implementation-handoff.md`.
- `rfcs/README.md`: added the `v0.21.3` entry under Implemented (Shipped:
  v0.21.3), removed it from Proposed (now lists only RFC-v0.21.3-002,
  matching its "1 RFC" header, which had been stale — it said "1" while
  showing 2 rows before this commit), retitled the Proposed subsection,
  updated the `done/` count (154 → 155 files, 153 → 154 RFCs).

## 4. Important implementation decisions

- **Left the 5 historical `review-request-*`/`review-record-*.md` files'
  `proposed/` references untouched.** They're dated submissions; the RFC
  genuinely was in `proposed/` when each was written. Rewriting their
  prose or links after the fact would misrepresent what was true at
  submission time, which is different from fixing a live document that's
  supposed to reflect current state. This was also the specific correction
  requested when my first attempt swept too broadly.
- **Superseded notes added in place, not rewritten as v0.21.3 claims** —
  consistent with the just-ratified ruling on the handoff bundle's version
  stamps (a frozen v0.21.2 snapshot should not be made to describe v0.21.3).
- **`rfcs/README.md`'s Proposed header/table mismatch (said "1 RFC", showed
  2 rows)** — noticed and fixed as part of this edit, since removing
  `v0.21.3-001` from that table was the natural place to also correct it.
  Not explicitly requested, but a one-line consequence of the requested
  change, not new scope.

## 5. Differences from the review record

None. Implemented exactly per review-record-slice-3-completion.md §5.

## 6. Executed commands and real output

```
$ cargo xtask build
0 warnings

$ grep -c "| OPEN |" docs/rfcs/ERRATA.md
0   (Gate 7 unaffected)

$ cargo xtask release-rehearsal
  [PASS] Gate 1  Host test suite (0 failures)
  [PASS] Gate 2  Unsafe audit (0 missing)
  [PASS] Gate 3  MMIO audit (0 missing)
  [PASS] Gate 4  ABI snapshot verify
  [PASS] Gate 5  Readiness matrix (0 OPEN)
  [PASS] Gate 6  Trust report (6 sections)
  [PASS] Gate 7  ERRATA register (0 OPEN)
  [PASS] Gate 8  Validation drills (markers)
  [ -- ] Gate 9  Release-notes limitations    MANUAL (unchanged)

# Link integrity, rfcs/README.md, re-verified after the refile:
total links: 158, missing: NONE
in done/ not linked: set()   |   linked but not in done/: set()
proposed/ mismatch: none in either direction
```

## 7. Unresolved issues and blocked items

None for this specific work. Per review-record-slice-3-completion.md §6,
what follows next is the actual v0.21.3 release cut under RFC-v0.21.3-002
(currently `Proposed`, not yet implemented/handed off) — including a
release record at `docs/release/records/0.21.3.md` and an owner decision
on Gate 10 (produce the record where Verus is installed, or tag with a
written accepted-risk statement). I have not started any of that; it
depends on RFC-v0.21.3-002 first, which is not mine to begin without a
handoff.

## 8. Known limitations

Unchanged from the prior submission. Nothing new introduced here.

## 9. Requested review focus

1. Is RFC-v0.21.3-001 now fully dispositioned, or is there a further step
   (e.g. an explicit closing note) expected?
2. Confirm the `rfcs/README.md` "Proposed — 1 RFC" header/count fix was
   the right call to fold into this commit rather than raise separately.
3. Whether to wait for an RFC-v0.21.3-002 handoff before touching anything
   related to the actual release cut, or whether any prep work is wanted
   in the meantime.
