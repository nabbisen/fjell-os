# RFC-0.29-002 §7 — Should the errata register have one parser?

**Governing RFC:** [rfcs/accepted/RFC-0.29-002-predicates-that-parse.md](../../rfcs/accepted/RFC-0.29-002-predicates-that-parse.md)

Answered before R1, per the handoff's required order — doing R1 first
would mean writing a parser that might then have to move.

---

## Answer: shape 1 — one shared parser, in `fjell-consistency-check`

At least four places used to re-parse `docs/rfcs/ERRATA.md`'s `## Summary`
table independently: Gate 7 (`grep -c "\| OPEN \|"`, the worst of the
four — an exact literal, not even a parser), `errata-limitations`'s own
`parse_accepted_errata`, `errata-tracking`'s own `parse_summary_rows`, and
the register's own hand-maintained totals row. Fixing Gate 7 alone would
have left three more free to drift from it and from each other — E-015's
family arriving by way of E-014's fix, exactly as the RFC's own framing
warned.

**Built:** `tools/fjell-consistency-check/src/errata.rs` (new), exposed via
a new `[lib]` target on that crate (it previously had only a `[[bin]]`).
`SummaryRow { id, tracking, status }`, `parse_summary_rows`, and three
structural predicates — `is_open`, `is_accepted`, `is_closed`, each
`status.trim_start().starts_with(...)` rather than an exact match, so an
annotated cell (`"OPEN (blocked on X)"`, the register's own established
shape for `ACCEPTED`) is read correctly.

**Every consumer now uses it:**
- `errata_tracking.rs` and `errata_limitations.rs` (same crate) import it
  directly — their own private copies of `parse_summary_rows`,
  `classify_tracking`, and `extract_erratum_id` are gone.
- `crates/fjell-tools` (a *different* crate) takes a path dependency on
  `fjell-consistency-check` specifically for this — Gate 7 now reads
  `docs/rfcs/ERRATA.md` directly and calls
  `fjell_consistency_check::errata::{parse_summary_rows, is_open}`,
  replacing the `grep` shell-out entirely.

## Shape 3, addressed on its merits, not dismissed for size

**Shape 3 — generate the table from structured data so there is nothing
left to parse — is the only shape that removes the problem rather than
centralising it, and it is not built here.** The RFC and its handoff both
named this as the architecturally better answer and both declined to cost
it; the same restraint applies here rather than pretending to have solved
what was flagged as unsized.

Why it is not attempted regardless: every erratum entry in `ERRATA.md` is
free-form prose (a `## E-NNN — ...` heading, a `**Claim:**`/`**Shipped:**`
narrative, review-dated blockquote addenda accumulated over months) with
one small structured table row embedded in the `## Summary` section at the
bottom. Moving to structured data means either:

1. **A second source of truth** (the structured data) that the prose must
   be kept in sync with by hand — reintroducing exactly the drift this
   line exists to remove, one level up; or
2. **Restructuring every existing entry's authoring format** so the prose
   itself becomes the structured data (e.g., YAML frontmatter per erratum,
   or a fully tabular register with prose in a separate field) — real
   design work with its own review cycle, and a rewrite of every one of
   the 38 entries currently in the file, not a fix this line's `Touches`
   covers (`crates/fjell-tools`, `tools/fjell-consistency-check`,
   `tools/fjell-unsafe-audit`, `docs/verification/instrument-audit.md`).

Shape 1 is not a rejection of shape 3 on principle — it is the answer that
fits inside this line's actual scope. If a future line wants to size shape
3, this module's `SummaryRow` is exactly the intermediate representation
such a migration would target: every consumer already reads through it,
so a structured-data backend could replace `parse_summary_rows`'s
implementation without touching a single call site.

## Why this isn't "one rule for the wrong reason"

Unlike RFC-0.28-005's/-0.29-001's "different jobs, different derivations"
answer, this really is one literal shared parser for one literal shared
document structure — and that is correct here specifically because all
four consumers are asking the *exact same question* ("what does this row
of this one table say"), unlike those two RFCs' walkers, which were
asking different questions of different data. There is no risk of this
being "shape 1 wearing a different shape's argument," because there was
never a case for a second, differently-shaped answer to make here.
