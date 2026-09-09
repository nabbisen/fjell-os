# Developer Handoff — RFC-0.29-002

**Governing RFC:** [RFC-0.29-002](../../accepted/RFC-0.29-002-predicates-that-parse.md)
**Milestone:** 0.29 — closes the audit backlog if it lands
**Status:** inherited from the governing RFC (Accepted, 2026-09-09)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Gate 7 first, and it is not a formality

Gate 7's job is to stop a release shipping with an open erratum. It runs
`grep -c "| OPEN |"`, and an erratum written `| OPEN (blocked on X) |` counts
zero. The register already writes one status cell that way — `E-004`'s reads
`| ACCEPTED (v1.0 limitation) |` — so this is a shape the format uses, not one
invented for the demonstration.

**It has never fired**, because nothing has been `OPEN` since the register
existed. So there is no regression risk and no baseline to preserve: whatever it
reports after the fix is the first true reading it has ever produced.

**If this line runs out of room, Gate 7 is the part that must land.** Everything
else on the list is a check that reports the wrong thing; this is the one that
lets a release proceed against its own rule.

## 0.1 Design decisions settled — do not re-open

1. **Parse, do not match** (D1).
2. **A better literal is not a fix** (D3). `"| OPEN |"` → `"| OPEN"` is a wider
   string. E-014's own text: *"each patch would be a better string."*
3. **Each fix demonstrated on the input the old predicate missed** (D4) — not on
   a generic broken input.
4. **E-017's four bare rows get demonstrations; its four proxy rows get their
   citations corrected** (D5). These are different actions and the register must
   not blur them.
5. **E-015's two lines are deleted or wired up** (D6), and E-015 closes.

---

## 1. Order

**§7 answered → R1 (Gate 7) → R2 (Gate 6) → R3 (the other five) → R4 (E-017) →
R5 (E-015) → close.**

§7 first because it decides whether the register gets one parser or four; doing
R1 before that answer means writing a parser you may then have to move.

## 2. §7 — and shape 3 is the one I cannot cost

**Should the errata register have one parser?** At least four instruments parse
`ERRATA.md` independently today. Fixing them separately yields four correct
parsers that can drift — E-015's family arriving through E-014's fix.

I lean to shape 1 (one shared parser). **Shape 3 — generate the table from
structured data so there is nothing to parse — is the only one that removes the
problem rather than centralising it, and it is much the largest.** I have not
costed it and will not pretend otherwise. If you think it is tractable, say so;
it would be the better answer.

## 3. Gate 6 is the one that gets skipped

`let _ = sh(&[…])` regenerates the trust report and **throws away whether that
succeeded**, then counts `§1`..`§6` in whatever file is on disk. A failed
regeneration leaves the previous report there, still holding six markers, and
the gate passes on a stale artefact.

It is fiddlier than the others and it is second in consequence only to Gate 7.
**Do not leave it for last and then run out of time.** The RFC's risk section
names this specific temptation.

## 4. R4 — two different actions, kept separate

E-017's rows split:

- **Four with no demonstration** — `fjell-abi-snapshot` ×2,
  `repro/baseline-digests.txt`, `ci-arm64-check`. These need a demonstration
  **produced**.
- **Four citing `cargo test -p <tool>`** — Gate 2, Gate 11, Gate 12,
  `syscall/expected.toml`. Real demonstrations for these **already exist** from
  RFC-0.24-002/003, 0.27-001/003/004 and 0.28-002/004. These need their
  **citations corrected**, not new work.

**Do not report the second group as demonstrations you produced.** Correcting a
citation to point at someone else's evidence is a smaller and different claim,
and conflating them is the shape of the erratum you are closing.

Also reconcile **21 rows vs the summary table's 22**. That table's arithmetic was
corrected once before, in the RFC-0.24-002 review; if it is wrong again, say
which cell.

## 5. Prohibited shortcuts

- Do not fix a literal with a wider literal.
- Do not skip Gate 6's exit-status handling.
- Do not claim a corrected citation as a produced demonstration.
- Do not change what any gate decides — only how it decides it.
- Do not close E-014/E-015/E-017 with an instance surviving. Name it and leave
  it open, as RFC-0.29-001 did with `v0.6-verification`.
- Do not touch the kernel, ABI, or any service.
- Do not run `cargo fmt --all --check` in your head, and put it in the evidence
  list.

## 6. Required evidence

1. **§7 answered in writing**, with shape 3 addressed on its merits.
2. **Gate 7 demonstrated** on `| OPEN (blocked on X) |` — failing before, seeing
   it after.
3. **Gate 6 demonstrated** on a regeneration that fails while a stale report
   remains on disk.
4. The other five, each demonstrated on the input it missed — including the real
   `TEST:M7:FAIL (init did not exit cleanly)` line for `FORBIDDEN`.
5. **E-017: four demonstrations produced, four citations corrected, listed
   separately**, and the 21-vs-22 count reconciled.
6. E-015's two lines gone.
7. **E-014, E-015, E-017 `CLOSED`** — or any survivor named and left open.

   **E-015's tracking field must be retracked deliberately.** It still reads
   `RFC-0.29-001`, because two live RFCs cannot both claim one erratum and
   `errata-tracking` refused the change — correctly, and while this RFC was
   being written to fix that very subcheck. Retrack it because it is right, not
   because a gate prompts you; the gate's predicate is one of the seven on the
   list and may not see this RFC's phrasing at all.
8. `release-rehearsal` green; `test-all` all tiers; `syscall-surface` 35/29/6;
   `callsite-audit` 5 checks.
9. `cargo fmt --all --check`.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **What Gate 7 reports once it can actually see.** If the count changes in
  either direction, that is the finding of the line.
- Your §7 answer, particularly on shape 3.
- The E-017 split — which rows you demonstrated versus cited, kept apart.
- Any instrument on the list that turned out **not** to have the defect.
- Any count of mine you re-derived and found different. Seven consecutive lines
  have corrected one, and last time it was a figure I had put in the required
  evidence.
