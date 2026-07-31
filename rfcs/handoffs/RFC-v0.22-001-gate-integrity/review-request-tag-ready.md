# Review Request — 0.22.0 ready for owner tag approval

**Governing RFC:** [rfcs/proposed/RFC-v0.22-001-gate-integrity.md](../../rfcs/proposed/RFC-v0.22-001-gate-integrity.md)
**Prior review:** [rfcs/handoffs/RFC-v0.22-001-gate-integrity/review-record-slices-1-4.md](../../rfcs/handoffs/RFC-v0.22-001-gate-integrity/review-record-slices-1-4.md) — **Approved**, both findings dispositioned by the architect
**Submitted by:** implementation model
**Status:** Both remaining implementer items from the review record's §7 done, plus one self-consistency fix. Release record rewritten fully green.

Branch: `docs/v0.21.3-rfc-and-design-baseline`

---

## 1. What this covers

The review record's §7 ("Answer to the open question") told me to proceed
and listed three remaining items. This request covers all three plus one
fix I found while doing them:

1. **Bumped `Cargo.toml` to `0.22.0`** (and `Cargo.lock` follows). Confirmed
   via `cargo metadata --no-deps`: all 89 workspace members report `0.22.0`.
2. **Added the dated `## [0.22.0]` CHANGELOG entry.**
3. **Re-ran the full exit-criteria sweep at this final state and rewrote
   `docs/release/records/0.22.0.md`** — BLOCKED marker removed, twelve-gate
   table recorded (all PASS), both findings documented under "Findings
   resolved" with the architect's dispositions, the v0.23 candidate from
   §6 carried into "Known limitations."
4. **Fixed a self-consistency gap in `docs/rfcs/ERRATA.md`** — your
   disposition commit (`50cb75d`) updated E-012's Summary-table row and
   the trailing paragraph to `ACCEPTED`, but the erratum's own per-entry
   `**Resolution:**` line still read `**OPEN**`. Updated it to match,
   with the same attribution (architect, 2026-07-31) and reasoning already
   given in the Summary section. This didn't affect any gate (the
   `errata-limitations` subcheck only reads the Summary table), but the
   document was internally contradicting itself, which is exactly the
   category of thing this RFC line exists to catch.

## 2. Evidence, this final state

- `cargo metadata --no-deps --format-version 1`: exit 0, 89 members, all
  `0.22.0`.
- `cargo fmt --all --check`: clean.
- `cargo xtask build`: full workspace build succeeds, `fjell-kernel
  v0.22.0`.
- `cargo xtask test-all --no-qemu`: 5/5 required tiers PASS.
- `cargo xtask test-all` (full): 18/18 PASS, zero regressions.
- `cargo xtask release-rehearsal`: **ALL MECHANICAL GATES PASS** across
  all twelve gates (Gate 9 manual, correctly unsigned) — independently
  re-run by me after your disposition commit, not just copied from your
  review record.

## 3. Not done, and why

**Did not apply the `0.22.0` tag.** Per RFC-v0.21.3-002's roles table and
your own review record §7 ("the tag remains the owner's"), that is the
owner's action:

```sh
git tag -s 0.22.0 -m "Fjell OS 0.22.0"
git push origin 0.22.0
```

**Did not act on the v0.23 candidate** (ACCEPTED being an unguarded escape
hatch, your review record §6) — explicitly marked not-decided there, and
I've carried it forward into the release record's "Known limitations"
table rather than picking a remedy myself.
