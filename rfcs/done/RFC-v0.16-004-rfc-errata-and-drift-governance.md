# RFC-v0.16-004: RFC Errata and Drift Governance

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review RB-05

---

## 1. Problem

The v0.9–v0.15 handoff (§6.1) identified nine RFCs whose normative text
claimed more than the merged implementation delivered. The RFC text was
treated as a forward-looking specification, not a backward-verifiable
record, and the divergences entered the freeze candidate silently. For a
project whose quality story rests on RFC discipline, undocumented drift
is itself a defect.

## 2. Change

1. **New status `Implemented-with-Errata`** added to the lifecycle policy
   (`rfcs/done/000-rfc-lifecycle-policy.md`). An RFC may not be marked
   `Implemented` if its normative text over-claims relative to merged
   code; it must be `Implemented-with-Errata` with a matching entry in
   the errata register.

2. **New status `Superseded`** added, with a required pointer to the
   successor RFC.

3. **New file `docs/rfcs/ERRATA.md`** — the standing drift register. Each
   entry names the RFC, the claim, what shipped, the resolution, and the
   tracking RFC.

4. **Drift rule:** no RFC may carry an over-claim into a release. Either
   the code is brought up to the claim, the claim is corrected, or the
   gap is recorded as an ACCEPTED limitation and surfaced in release notes.

## 3. Initial population

The nine drift items from handoff §6.1 are recorded as E-001 … E-009.
Eight are CLOSED by v0.16 RFCs; one (E-004, hardware boot) is ACCEPTED as
a disclosed v1.0 limitation per RFC-v0.16-005.

## 4. Standing requirement: cryptographic test vectors

Triggered by E-001's root cause (a hand-transcribed seed corrupted from
byte 15): any committed cryptographic test vector must be cross-verified
against at least one independent implementation at authoring time, and
the verification command recorded in the test module or its RFC. This
prevents a self-consistent-but-wrong vector from masquerading as
conformance.

## 5. Release gate

The release checklist (RFC-v0.15-003) gains a step: **ERRATA register
shows 0 OPEN items.** ACCEPTED items are permitted but must each appear
in the release notes' limitations section.

## 6. Test plan

- `docs/rfcs/ERRATA.md` exists and parses as a table.
- Every tracking RFC referenced by a CLOSED entry exists in `done/`
  or `proposed/`.
- The release checklist references the errata gate.
