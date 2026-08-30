# RFC-0.27-003: The standards mapping — a claims document, in a project whose claims keep turning out to be unchecked

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.27
**Tracks.** BIZ-06 from the system proposal: a maintained mapping from CRA
Annex I and IEC 62443-4-1/4-2 to Fjell mechanisms and evidence artifacts, with
per-clause status.
**Touches.** `docs/compliance/` (new), `tools/fjell-consistency-check`,
Gate 12's subcheck count. **Does not touch the kernel, the ABI, or any service.**
**Relates to:** **E-027** (a gate asserted in published documentation that was
never built — this RFC's failure mode, already realised once); **E-023** (four
of five specified behaviours never built while the RFC read `Implemented`);
R-5 in the system proposal, re-rated to *medium* probability on 2026-08-31.

## Summary

BIZ-06 asks for the cheapest artifact in the proposal: a table mapping
regulatory clauses to the mechanisms and evidence this project already
produces. No new mechanisms, no kernel work, documentation only.

**It is also the most dangerous document this project has ever been asked to
write**, and the reason is in the errata register rather than in the
regulation. Every row of a standards mapping is a sentence of the form *"this
requirement is satisfied by that mechanism, evidenced by this artifact."* That
is precisely the sentence shape this project has spent four milestones finding
to be false in its own documentation:

- **E-023** — an RFC read `Implemented` while four of its five specified
  behaviours had never been written.
- **E-027** — a published document asserted a "threat-model gate" that has
  never existed in any commit on any branch.
- The 0.24 instrument audit — **33 findings against 22 sound instruments,
  every one of which was reporting green.**

Those were internal documents, read by two people. This one is written for
assessors, buyers and design partners, and a false row in it is a
misrepresentation to a third party rather than an embarrassment.

**So the mapping is not the deliverable. The mapping plus the instrument that
keeps it honest is the deliverable**, and this RFC treats the second as the
load-bearing half.

## Motivation

### Why now, stated accurately

The system proposal's §2.1 is right that the CRA is a forcing function, and the
correction pass on that document notes 11 September 2026 is 11 days out. **Be
precise about what that date is and is not**, because overstating it here would
be the same defect this RFC exists to prevent:

- The 11 September 2026 obligation is **reporting of actively exploited
  vulnerabilities**, and it binds **manufacturers placing products on the EU
  market**. Fjell OS is not placed on the market, is pre-1.0, and has no
  commercial adopter. **Nothing legally obliges this project on that date.**
- What the date does is set the **buyer's** planning horizon. Manufacturers
  evaluating components in late 2026 are doing so under a compliance clock, and
  the mapping is the artifact that makes Fjell legible to them.

The urgency is commercial and real. It is not legal, and the RFC says so in
writing so that nobody later reads urgency into an obligation that does not
exist.

### What already exists to map

More than the proposal credits. The mapping is largely a re-presentation of
artifacts that ship today: the 12 release-rehearsal gates, 21 test tiers, the
threat model's T1–T20 and OS1–OS8, `docs/release/v1-limitations.md`, the errata
register, `trust-report.txt`, the unsafe and MMIO audits, the ABI snapshot, the
reproducible-build baseline, the Verus proofs, and `.github/SECURITY.md` as
corrected on 2026-08-31.

That is the good news and the trap in the same sentence. A mapping assembled
from documents that already exist will look complete on first draft, and
completeness is not what a correct mapping looks like.

## The settled part — decisions not to be re-opened

**D1 — This document does not claim conformity, and must not be readable as
claiming it.** It maps mechanisms to clauses and states evidence. Only a
conformity assessment can establish conformity. The words "compliant",
"certified", and "conformant" do not appear applied to Fjell. The document
opens with what it is not. This is the project's largest overclaim surface to
date and R-5 is now rated *medium probability, fatal impact*.

**D2 — Clause text is transcribed from the official source, with a citation and
a retrieval date. Working from memory or from a secondary summary is
forbidden.** The CRA is Regulation (EU) 2024/2847 and Annex I is publicly
available. An LLM's recollection of clause numbering is exactly the
"asserted without checking" failure this RFC is about, aimed at a legal text —
where a wrong clause number is not a typo but a false statement about the law.
If the official text cannot be retrieved, the mapping is not written.

**D3 — IEC 62443-4-1 and 4-2 are paywalled and must not be transcribed.** They
are sold by the IEC; reproducing clause text would be a licensing violation,
not merely poor practice. Until the owner decides to purchase them, the 62443
half maps to the **publicly documented structure only** — the seven foundational
requirements and the 4-1 practice areas, by identifier and title — and is
explicitly labelled *structural, not clause-level*. Do not paraphrase a
paywalled clause and present the paraphrase as its content. **A structural
mapping honestly labelled is worth more than a clause-level one that cannot be
sourced.**

**D4 — The status vocabulary is closed, and absence is fail-closed.**
`met` / `partial` / `not-met` / `not-applicable` / `roadmap`. A row with no
cited artifact is **`not-met`**, never `partial`. A row whose evidence is "the
architecture makes this true" with nothing to point at is `not-met`. Reading
absence of evidence as presence is mode 3 of this project's own defect
taxonomy, and it is the way a compliance document rots.

**D5 — A first draft that is mostly `met` is evidence of a bad mapping.** Many
clauses will be `not-applicable` (Fjell is a component, not a product with a
user interface or personal data processing) and many will be `not-met` or
`roadmap` (SBOM emission is BIZ-01 and unbuilt; the support lifecycle is BIZ-03
and undecided). **The reviewer will weight a high `met` count as a defect
signal, not a success signal**, and the implementer should expect that.

## The actual deliverable — R1…R5

**R1 — `docs/compliance/standards-mapping.md`.** One document, two parts: CRA
Annex I (Parts I and II) clause-level; IEC 62443-4-1/4-2 structural per D3.
Every row: identifier, requirement (sourced per D2/D3), status per D4, the Fjell
mechanism, and **a repository-relative path to the evidence**.

**R2 — every `met` and `partial` row cites at least one path that exists.**
This is the row-level contract the instrument enforces.

**R3 — a new `standards-mapping` subcheck in `fjell-consistency-check`**, taking
Gate 12 from **8 subchecks to 9**. It verifies, over the mapping document:

1. every status cell is from D4's closed vocabulary;
2. every `met` or `partial` row cites at least one path;
3. every cited path exists in the tree;
4. **direction B** — no row cites a path that does not exist, and no row is
   silently statusless.

**R4 — the subcheck demonstrated failing**, per RFC-v0.22-001, on four
deliberately broken inputs: an invented status value; a `met` row with no path;
a row citing a deleted file; a row with an empty status cell. A subcheck that
has only ever been seen passing is `UNAUDITED`, not sound.

**R5 — the mapping is named in the release cycle** as an artifact to re-verify
at the cut, alongside `v1-limitations.md`. Gate 12 covers it mechanically; the
cut covers whether it still *says the right thing*.

## The open question — §4

**Does a `not-met` row block a release?**

`v1-limitations.md` and the errata register both take the position that an
honest disclosure does not block; only `OPEN` errata do (Gate 7). A mapping
full of `not-met` rows is, by D5, the expected and correct first state.

Three shapes:

1. **Never blocks.** The mapping is disclosure, like `v1-limitations.md`. Risk:
   nothing ever creates pressure to close a row.
2. **A row may not silently *regress*** — `met` → `not-met` between releases
   fails the cut unless the change is recorded, the way an ABI removal must be
   reconciled rather than absorbed. Risk: needs a stored baseline, which is a
   second artifact and a second thing that can go stale.
3. **Blocks on `met` rows whose evidence disappeared** — the narrow case, which
   R3's check already detects; the only question is whether it is fatal or
   reported.

**Answer this in writing before implementing R3**, and say which of the three
the subcheck implements. The architect's inclination is 3, with 2 recorded as
the next line's work — but this is the implementer's to argue.

## Scope

`docs/compliance/standards-mapping.md`; `tools/fjell-consistency-check`;
`docs/src/release/v0-release-cycle.md`; `rfcs/README.md`; `docs/rfcs/ERRATA.md`
if the mapping surfaces a gap worth a row.

### Non-goals

- **Building any mechanism a clause asks for.** A gap becomes a `not-met` or
  `roadmap` row and, if structural, an erratum. It does not become a rushed
  implementation inside a documentation RFC.
- **BIZ-01 (SBOM) and BIZ-03 (support lifecycle).** Both will appear as
  `not-met` rows. BIZ-03 is an owner decision this RFC must not pre-empt.
- **Purchasing or transcribing IEC 62443** (D3).
- **Legal advice or conformity assessment** (D1).
- Any change to the kernel, the ABI, the syscall surface, or any service.
- Resolving **E-024**, **E-025**, **E-026** or **E-027**, though the mapping
  will and should cite them where a clause touches what they record.

## Risks

**The mapping is a marketing document wearing an engineering document's
clothes.** Every incentive pushes toward more `met` rows. D4, D5 and R3 exist
against that pressure; the review will read the `not-met` rows first, and a row
upgraded from `not-met` to `met` without a new artifact will be treated as a
finding.

**Nothing checks that a cited artifact still *says* what the row claims.** R3
verifies a path exists, not that the file supports the assertion. This is a
weak predicate (mode 4), it is deliberate, and it must be **disclosed in the
mapping document itself** rather than left for a reader to discover.
