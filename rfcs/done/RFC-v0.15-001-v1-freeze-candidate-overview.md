# RFC-v0.15-001 — v1.0 Freeze Candidate Overview

**Status:** Implemented (v0.15.0)
**Target version:** v0.15.0
**Parent:** RFC 061 §10.
**Cross-refs:** v0.15-002 through v0.15-005, all earlier milestones.

## 1. Purpose

v0.15 is the last milestone before v1.0. It is not an architecture
milestone. It is a **discipline milestone**: the system stops growing
in surface, the threat model is finalised in writing, the release
process becomes mechanical, the operator guide becomes the authoritative
reference, and the constraints that define v1.0 are locked.

After v0.15, anything that lands before v1.0 is a *fix*. Anything new
is post-v1.0.

The bar:

> An outsider auditing the Fjell repository can determine, without
> reading source, what v1.0 promises, what v1.0 explicitly refuses to
> promise, how to verify any promise mechanically, and how to recover
> from any catalogued failure.

If that bar is met, v1.0.0 is tagged.

## 2. Composition

| RFC | Title | Deliverable |
|-----|-------|-------------|
| v0.15-001 | This overview | Coordination |
| v0.15-002 | Threat Model Finalization | `docs/security/threat-model-v1.md` |
| v0.15-003 | Release Checklist and Security Advisory Process | `docs/release/release-checklist.md` + advisory policy |
| v0.15-004 | Operator Recovery Guide and Field Documentation | `docs/operations/recovery-guide.md` |
| v0.15-005 | v1.0 Non-Goals and Constraint Lock | `docs/release/v1-non-goals.md` |

## 3. Posture: the freeze

Between the start of v0.15 development and the v1.0.0 tag, this RFC
imposes the following discipline:

- **No new RFCs in `proposed/` except for fixes.** A "fix" is a change
  whose acceptance criterion is "this previously worked / was claimed
  to work, and is now restored / made true."
- **No new stable surface.** Provisional items in v0.10-002 can move
  to STABLE or remain provisional; new items are deferred.
- **No new catalog tags.** The semantic catalog is frozen at v1.
- **No new CapKind variants.** Existing kinds can be reorganised but
  not extended.
- **No new fleet protocol additions.** The wire format is locked.
- **Documentation may freely improve.** Docs are not part of the
  freeze.

Violations of the freeze require a v0.15 RFC explicitly authorising
the exception with a stated reason; the RFC must reference this one.

## 4. What v1.0 actually means

After this milestone, "v1.0" denotes:

1. The identity from RFC 061 §2 is in force.
2. The eight invariants I1–I8 hold.
3. The stable surface from RFC-v0.10-002 §2 is committed.
4. The v1.0 readiness matrix (RFC-v0.10-007) has zero OPEN cells.
5. The deployment profile from RFC-v0.12-002 is supported.
6. The recovery scenarios from RFC-v0.13-005 have working playbooks.
7. The Trust Report has six populated sections (RFC 061 §6).
8. The signing key for v1.0.0 has documented provenance.

These are the eight v1.0 propositions. Any one of them being false
disqualifies the release.

## 5. Release criteria

v0.15.0 may be tagged when:

1. The four sub-RFCs (002–005) are merged to `done/`.
2. The freeze discipline (§3) has been in force throughout the cycle
   without unauthorised exception.
3. `docs/security/threat-model-v1.md` covers every adversary class
   the team commits to mitigating.
4. `docs/release/release-checklist.md` produces a reproducible release
   when executed verbatim.
5. The advisory process from v0.15-003 has been exercised once
   end-to-end against a synthetic advisory.
6. The recovery guide from v0.15-004 covers every entry in v0.13-005's
   DR table.
7. `docs/release/v1-non-goals.md` is committed and survives an
   adversarial review (a contributor attempts to negotiate items off
   the list; the document holds or the negotiation is reflected back
   in the doc).

v1.0.0 may be tagged when, in addition:

1. All eight v1.0 propositions from §4 are true.
2. The v1.0 readiness matrix has zero OPEN cells.
3. The release checklist runs to completion against the v0.15 tree
   and produces signed artefacts.

## 6. Risk register

| Risk | Mitigation |
|------|------------|
| Last-minute "small" features creep in | §3 freeze with required exception RFCs |
| Threat model becomes a marketing exercise | v0.15-002 lists adversaries by capability, not category |
| Release checklist drifts from reality | v0.15-003 requires one rehearsal of the full procedure |
| Recovery guide is correct but unreadable | v0.15-004 requires a person uninvolved with the writing to follow it once |
| Non-goals doc invites endless debate | v0.15-005 is itself an authority document — debate it once, then closed |

## 7. After v1.0

This RFC explicitly does not commit Fjell to anything beyond v1.0.
Decisions about v1.1, v2.0, LTS branches, contributor invitation,
external authoring programmes, multi-fleet federation, post-quantum
hybrid mode, and second-platform targets are deferred to a future
strategic RFC after v1.0 ships.

The deferral is itself a v1.0 commitment: nothing about v1.0's posture
is contingent on plans that do not yet exist.

## 8. Out of scope

- Anything resembling new functionality.
- v2.0 planning.
- Public marketing for the v1.0 release.
- Conference / journal / publication submissions about Fjell. Those
  may proceed independently but are not part of the release work.
- A contributor governance change. Solo-author project remains so
  through v1.0.
