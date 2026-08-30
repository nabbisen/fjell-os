# Developer Handoff — RFC-0.27-003

**Governing RFC:** [RFC-0.27-003](../../proposed/RFC-0.27-003-standards-mapping.md)
**Milestone:** 0.27
**Status:** inherited from the governing RFC (Proposed — awaiting owner acceptance)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The one that will get you, before anything else

You are about to write, roughly a hundred times, a sentence of the form
*"requirement X is satisfied by mechanism Y, evidenced by artifact Z."*

**You already know most of this project. You will be able to write those
sentences from what you know.** Do not. Every one of them is checkable, and the
only reason this project has an errata register with 27 entries is that people
wrote sentences of exactly that shape without checking them — including me,
twice, in consecutive lines.

**Open the artifact. Confirm it says what the row claims. Then write the row.**
If you cannot open it, the row is `not-met`.

## 0.1 Two things that would be worse than a bad row

**Getting a legal clause number wrong.** The CRA is Regulation (EU) 2024/2847;
Annex I is publicly available. **Transcribe from the official text with a
citation and a retrieval date.** Do not write clause numbering or clause content
from recollection, and do not work from a blog summary or a vendor whitepaper.
A wrong CRA clause number in a buyer-facing document is a false statement about
the law, not a typo, and it is the single most damaging thing this line could
ship.

If you cannot retrieve the official text, **stop and say so.** A missing CRA
section is recoverable. A confidently wrong one is not.

**Transcribing IEC 62443.** 62443-4-1 and 4-2 are sold by the IEC. **Do not
reproduce clause text, and do not paraphrase a clause you have not read and
present the paraphrase as its content.** Map to the publicly documented
structure only — the seven foundational requirements and the 4-1 practice areas,
by identifier and title — and label that half **"structural, not clause-level"**
in the document itself. An honestly-labelled structural mapping is worth more
than a clause-level one nobody can source, and it is the only lawful option
until the owner buys the standards.

## 0.2 Design decisions settled — do not re-open

1. **No conformity claim** (D1). The words "compliant", "certified" and
   "conformant" do not appear applied to Fjell. Open the document with what it
   is not.
2. **Closed status vocabulary, fail-closed** (D4): `met` / `partial` /
   `not-met` / `not-applicable` / `roadmap`. **No artifact ⇒ `not-met`.** Never
   `partial` as a way of avoiding `not-met`.
3. **A mostly-`met` first draft is a defect signal** (D5). Expect many
   `not-applicable` (Fjell is a component, not a product with a UI or personal
   data) and many `not-met` or `roadmap`. BIZ-01 (SBOM) is unbuilt; BIZ-03
   (support lifecycle) is an owner decision you must not pre-empt — both are
   `not-met` rows.
4. **No mechanism gets built here.** A gap is a row, and if structural, an
   erratum. Not a rushed implementation inside a documentation RFC.

---

## 1. Order

**R1 draft → §4 answered → R3 subcheck → R4 demonstrations → R2 reconciled → R5.**

Draft the mapping first, because you cannot design the checker until you know
what the rows look like. But **answer §4 before writing R3** — whether a
`not-met` row blocks a release determines what the subcheck does when it finds
one, and retrofitting that decision into a written checker is how the decision
gets made by accident.

## 2. §4 is a real question, not a formality

**Does a `not-met` row block a release?** Three shapes in the RFC. My
inclination is shape 3 — block only when a `met` row's evidence has
disappeared — with shape 2 recorded as the next line's work. **Argue it; do not
adopt it because I said it.** The last two lines both overturned something I
had written, and both were right to.

## 3. R3 — what the subcheck must and must not claim

It verifies four things (RFC §R3). **It does not verify that a cited artifact
supports the claim in its row** — only that the path exists. That is a weak
predicate, it is deliberate, and **it must be disclosed in the mapping document
itself**, in the document's own words, not left in this handoff. A reader who
sees a green gate must be able to find out what the gate does not cover.

Gate 12 goes **8 subchecks → 9**. Its label enumerates every subcheck by name;
the last time a subcheck was added, the label said four while running eight and
I caught it in review. Update the label in the same commit.

## 4. R4 — four demonstrations, all four required

| # | Broken input | Must |
|---|---|---|
| 1 | invented status value (`mostly-met`) | FAIL, naming the row and the bad value |
| 2 | `met` row with no cited path | FAIL, naming the row |
| 3 | row citing a deleted file | FAIL, naming the row and the path |
| 4 | row with an empty status cell | FAIL — **not silently skipped** |

4 is the one that matters. A row-parser that skips malformed rows reports `PASS`
on a document it did not read, which is mode 1 of this project's taxonomy and
the reason Gate 5 could not see a `**BLOCKED**` row.

## 5. Prohibited shortcuts

- Do not write a clause number or clause text from memory.
- Do not transcribe or paraphrase IEC 62443 text.
- Do not use `partial` where the honest answer is `not-met`.
- Do not build a mechanism to turn a `not-met` row green.
- Do not let the subcheck skip a row it cannot parse.
- Do not claim conformity, certification, or compliance.
- Do not touch the kernel, the ABI, the syscall surface, or any service.
- Do not run `cargo fmt --all --check` in your head.

## 6. Required evidence

1. `docs/compliance/standards-mapping.md`, with sources and retrieval dates.
2. **§4 answered in writing**, with the two rejected shapes and why.
3. The subcheck, with unit tests.
4. **All four demonstrations, captured** — the FAIL output for each.
5. Gate 12 label updated to name all **9** subchecks.
6. The mapping named in `docs/src/release/v0-release-cycle.md` (R5).
7. `release-rehearsal` green; `test-all` 21/21; `syscall-surface` **35/29/6**
   (unchanged — this RFC touches no syscall).
8. `cargo fmt --all --check` — run it.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Every `met` row**, with the artifact you opened to justify it. I will read
  these hardest and I will spot-check them against the tree.
- **Every row where you were tempted to write `partial`** and what made you
  choose one way or the other.
- Anything in the CRA text that does not map cleanly. A clause that resists the
  mapping is a finding about the product, and the most valuable output this
  line can produce.
- Anything you could not source, and what you did instead.
