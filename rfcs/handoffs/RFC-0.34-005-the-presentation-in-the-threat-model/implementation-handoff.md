# Developer Handoff — RFC-0.34-005

**Governing RFC:** [RFC-0.34-005](../../accepted/RFC-0.34-005-the-presentation-in-the-threat-model.md)
**Milestone:** 0.34
**Status:** inherited from the governing RFC (Accepted, 2026-10-13)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This line produces prose, which makes it the easiest to do badly

A threat model is the one artefact in this project that **cannot fail a gate for
being wrong**. `consistency-check` will confirm the citations resolve; nothing will
confirm a defence exists. So the measure of this line is not that the document gained
sections — it is that **every defence it claims names the tier, test or erratum that
demonstrates it**, and every defence that does not exist is a residual with a name
(§E, D5).

If you find yourself writing a sentence you could not be shown to be wrong about,
that is the sentence to replace with a residual.

## 0.1 Settled — do not re-open

1. Sender, receiver, **and absence/crash** — all three directions (D1).
2. The separate-device configuration is **named or explicitly excluded** (D2).
3. The model's existing shape; **T1–T20 are not rewritten** (D3).
4. **T17 is the hinge**, and its residual gains a pointer back (D4).
5. A missing defence is a **named** residual (D5).
6. Whatever `v1-limitations.md` or `standards-mapping.md` must change lands in the
   same commit, with the rows named for criterion 8 (D6).

## 1. Order

**R1 (re-derive, with the control) → §A–§E in writing → R3 (the threats) → R4 (T17's
pointer) → R5 (defences cited, residuals named) → R6 (the other documents) → R7
(E-060) → evidence.**

**§A–§E before prose.** Deciding how many threats there are, and what each one is,
after writing the sections is how a model acquires sentences that exist to fill a
heading.

## 2. What to read before writing a word

- `threat-model-v1.md` end to end — 230 lines, T1–T20 — and **T17 twice**: its
  defence is the attestation this component relies on, and its residual already
  describes the gap you are filling.
- `releases/0.33.0.md`'s *What shipped* for the presentation line, so Finding 3 is
  read from the record rather than remembered.
- `rfcs/done/RFC-0.34-001-a-second-presentation.md` and its review record for what
  E-058's fix actually guarantees — **the stream never initiates a blocking call to a
  presentation** — and for the four survivors it names.
- `docs/src/identity/v1-direction.md`'s archetype, which is what makes a proxy a
  component something depends on rather than a demonstration.
- `docs/src/adr/ADR-v0.5-005-proxy-text-no-input.md`, including the dated correction
  that the input route it names does not exist.

## 3. §B is the question that can make this line too big

If the answer is *"a presentation may run on a different machine"*, the model stops
being about one node and starts being about a channel — authentication, replay,
confidentiality, device identity. **That is a larger analysis than this amendment.**
Name the configuration, state that the model is not authoritative for it, and
**escalate**; do not write a thin version of it.

## 4. Prohibited

- Rewriting or re-opening T1–T20 (D3).
- **Building** any defence the amendment names as missing — naming it is the work.
- A defence with no citation (§E). If nothing demonstrates it, it is a residual.
- An invented adversary class where an existing one honestly fits (§D) — and if you
  do define one, say what makes it distinct.
- Touching the input path, or implying one exists.
- A citation that resolves on disk but not from the published site (RFC-0.33-004 D4).

## 5. Required evidence

1. R1: the three absences with the `T17` control, and Finding 3 read from the record.
2. §A–§E in writing, with §B's configuration answer explicit.
3. The new threats, each in the model's shape, each saying whether it is an instance
   of T17's adjacent class.
4. T17's residual pointing at them.
5. **A table of defences and what demonstrates each** — tier, test or erratum — and a
   list of residuals, each with its name.
6. The rows changed in `v1-limitations.md` / `standards-mapping.md`, **named for the
   architect's criterion-8 reading**.
7. **E-060** resolved or survivors named, register and limitations in one commit.
8. The gates, each by **its own exit status**, and `consistency-check --all` green
   including `standards-mapping` and `evidence`.

## 6. Review request

Standard format, in `.git-exclude/review-request/`. Flag for focused review:

- **The defence-to-evidence table**, first. It is what makes this amendment more than
  prose.
- **§B's answer**, and whether you hit the escalation in §3.
- Every residual you named, and the erratum or limitation each points at.
- Anything in T1–T20 you believe is now false — **report it, do not edit it.**
- Anything outside the *Touches* list, which is indicative.
