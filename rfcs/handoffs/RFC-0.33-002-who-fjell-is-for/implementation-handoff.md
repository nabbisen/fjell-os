# Developer Handoff — RFC-0.33-002

**Governing RFC:** [RFC-0.33-002](../../accepted/RFC-0.33-002-who-fjell-is-for.md)
**Milestone:** 0.33
**Status:** inherited from the governing RFC (Accepted, 2026-09-23)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

**This line changes documents, not code.** No crate, no test, no gate logic.

---

## 0. The measure is a reader who can tell who this is for

Today they cannot: the two pages a reader meets first do not mention inclusion,
while the requirements chapter of the same book lists accessible-UI devices as a
**primary** target. **When this line is done, both halves say the same thing, and
the limits are on the page beside the goal.**

**The trap in this line is enthusiasm.** Every sentence you add is a claim, and
the audience it would mislead is people who have been promised accessibility
before. D6's limitations section is not decoration — it is the condition on D1.

## 0.1 Re-derive first (R1)

Read the originals in `.git-exclude/specs/` — they are untracked, so quote what
you need into the tree rather than linking them:

```
/usr/bin/grep -n 'inclusion\|ABDD\|accessible' .git-exclude/specs/fjell-os-requirements-v1-20260504.md | head
/usr/bin/grep -rn 'accessib\|ABDD\|inclusion' docs/src/intro/                    # expect nothing
/usr/bin/grep -n 'headless edge/fleet nodes' docs/src/releasing/v1-non-goals.md   # N3's rationale
```

**Use `/usr/bin/grep -a`, not the shell's `grep`** — it silently skips files
containing a NUL byte, which is how a wrong figure reached three records this
month (E-055). Control every absence on the specific file the claim is about.

## 0.2 Settled — do not re-open

D1 inclusion is a primary goal, stated as goal and mechanism, never as delivery;
D2 §4.5 (no GUI stack in the core) stands unchanged; D3 §4.1 re-stated to mean
what it says; D4 N3's rationale and the identity list corrected; D5 archetype
**A4**, written like A1–A3; D6 the limitations section; D7 no conformance claim;
D8 documents only; **D9 three rows in the readiness matrix, marked
`**IN PROGRESS** → v1.x` and never `**OPEN**`**; D10 the roadmap says v1.x.

---

## 1. Order

**R1 → §A–§D answered in writing → D6 (the limitations section) → D1/D4/D5 (the
pages) → D3 (§4.1 and N3) → D9 (the matrix) → D10 (the roadmap) → R6 (E-054) →
evidence.**

**The limitations section is written first, before the claim it qualifies.** If
it turns out you cannot write an honest list of what a person needing speech or
braille cannot do today, then D1 is not ready to be written either.

## 2. A4 must be a node, not an aspiration

A1–A3 each name a concrete deployment with a concrete operator. **A4 does the
same or it does not belong:** what the node is, who operates it, through what
presentation, and what it does when the presentation is unavailable. If you
cannot write it that concretely, say so — that is a finding about the archetype,
not a licence to write something vaguer.

## 3. The readiness rows (D9), and the instrument that reads them

Three rows, as release criteria: a second presentation modality end to end; a
decided input path (an ADR, not necessarily an implementation); the
accessibility limitations section kept true at each cut.

- **Marked `**IN PROGRESS** → v1.x`.** `readiness-check` counts `**OPEN**` as
  blocking (Gate 5); `**IN PROGRESS**` does not block. Run it before and after
  and quote both counts.
- **Each bar is the row's own text**, not a comment beside it: the row is what a
  cut reads.

## 4. What must not happen

- **No conformance claim** — not EN 301 549, not Section 508, not WCAG, not
  "accessible" as a verdict about the product. Architecture and roadmap only.
- **Do not soften §4.5.** No GUI rendering stack in the core is what makes
  presentation a proxy's job.
- **Do not decide §A** (whether personal computing becomes a long-term goal).
  The RFC leaves it open deliberately; if you have an opinion, write it in the
  answer document as an opinion.
- **Do not touch code**, including the proxies. A second modality is 0.34.
- **Do not edit the specs under `.git-exclude/`** — they are the historical
  record of what was asked for.
- Do not run `cargo fmt --all --check` in your head.

## 5. Required evidence

1. R1's re-derivation, with `/usr/bin/grep -a` and a control per absence.
2. §A–§D answered in writing.
3. **D6's limitations section**, written before the pages that make the claim.
4. The intro pages, the identity document, N3's rationale, and A4.
5. §4.1 re-stated; §4.5 untouched (show the diff is empty for it).
6. D9's three rows, with `readiness-check` counts before and after, and Gate 5
   still green.
7. D10: `ROADMAP.md` at v1.x, the adaptive Personal Proxy recorded as
   unscheduled rather than dated.
8. E-054 CLOSED or its survivors named; register and `v1-limitations.md` in the
   same commit.
9. The book builds under the pinned mdBook; the **published site** checked for
   the changed pages (RFC-0.32-003 R8's shape), including that A4 is reachable
   from the navigation.
10. `consistency-check --all` **by exit status**; `cargo fmt --all --check`; a CI
    run id.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **D6's limitations section**, first — it is the sentence I will read hardest.
- **A4**, and whether you could write it as concretely as A1–A3.
- Any place where the founding requirements and the current book disagree beyond
  the four the RFC names.
- Anything you could not say without claiming more than the tree supports.
