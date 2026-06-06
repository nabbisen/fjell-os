# RFC-v0.15-004 — Operator Recovery Guide and Field Documentation

**Status:** Proposed
**Target version:** v0.15.0
**Parent:** v0.15-001.
**Cross-refs:** RFC-v0.12-005 (deployment notes), v0.13-003 (key
    compromise), v0.13-005 (disaster recovery).

## 1. Problem

v0.12-005 documents *deploying* Fjell. v0.13-003 and v0.13-005
document *recovering* from key compromise and disaster scenarios.
These are excellent operational documents but they were written
incrementally, scattered across milestones, and assume the reader has
read each prior milestone's RFCs.

A v1.0 operator should not need to read the RFC sequence. They need
one document — the **recovery guide** — that covers every catalogued
failure mode in order of probability, with self-contained procedures
that link out only when necessary.

v0.15 consolidates and audits.

## 2. The recovery guide

`docs/operations/recovery-guide.md` — the authoritative operator
reference. Structure:

### 2.1 Quick-start triage

A one-page decision tree that takes an operator from "something is
wrong" to the right section:

```text
A node is not booting       → §3.1  Boot failures
A bundle deployment failed  → §3.2  Rollout failures
A node has been quarantined → §3.3  Node quarantine
The coordinator is down     → §3.4  Coordinator loss
A key is compromised        → §3.5  Key compromise
The fleet is partitioned    → §3.6  Partition handling
Audit storage is corrupted  → §3.7  Local audit recovery
None of the above           → §4    General diagnostic flow
```

### 2.2 Per-scenario procedures

Each `§3.x` section is self-contained:

- Symptoms (what the operator sees).
- Diagnostic commands to run.
- Possible root causes ordered by likelihood.
- For each cause: containment + recovery + verification.
- Cross-references to RFCs only for justifying *why* the procedure
  works.

The procedures duplicate content from v0.12-005 / v0.13-003 / v0.13-005
deliberately. The recovery guide is a *destination*; the milestone
RFCs remain available for design-level questions.

### 2.3 Concrete commands

Every command is in a `bash run-verified` block (RFC-v0.10-006), so
drift between the guide and the implementation is caught by
`cargo xtask docs build`.

Example excerpt (illustrative, not exhaustive):

```text
§3.2  Rollout failure

Symptoms:
  - cargo xtask fleet status shows nodes in `RolledBack` state
  - FLEET.ROLLOUT_PAUSED audit record present
  - Trust Report shows recent rollback events

Diagnostic:
  cargo xtask fleet rollout status --plan-id <id>

Likely cause: health-check timeout
  Action:
    1. cargo xtask fleet rollout logs --plan-id <id> --node <bad-node>
    2. Inspect serial log for BUNDLE_HEALTH:FAILED reason code
    3. If reason is in the known list (table 3.2.1):
         cargo xtask fleet rollout rollback --plan-id <id>
       Else:
         escalate per §6 incident escalation
  Verification:
    cargo xtask trust-report verify --plan-id <id>
```

### 2.4 Failure-mode catalogue

The guide ends with a flat table listing every catalogued symptom:

| Symptom | Section | Severity | Estimated MTTR |
|---------|---------|----------|----------------|
| `FJELL-BOOT-FAIL: DTB` | §3.1 | High | 15 min |
| `BUNDLE_HEALTH:FAILED` | §3.2 | Medium | 30 min |
| `RECONCILE.REJECTED` | §3.6 | Medium | 1 hr |
| `SECURITY.ATTEST_REPLAY_REFUSED` | §3.5 | High | per playbook |
| `CONSISTENCY.SUMMARY_REJECTED` | §3.7 | Low | per playbook |
| `BOOT.DTB_MISMATCH` | §3.1 | High | varies |
| ... | ... | ... | ... |

Severity follows the v0.15-003 advisory rubric for consistency.
Estimated MTTR is operator guidance, not a contract.

## 3. Follow-test discipline

The guide must be followable by a person *not* involved in writing
it. v0.15 requires:

- One follow-test pass before landing: a designated reviewer who has
  not edited the guide walks one entry per section against a live
  reference fleet and reports gaps.
- Gaps are closed before landing or marked explicitly as TODO with a
  follow-up RFC reference.

The follow-test attestation is committed at
`docs/operations/recovery-guide-attestation.md`.

## 4. Trust Report integration

The Trust Report's "Recovery posture" subsection (introduced by
v0.13-005 §7) gains a column referencing the recovery-guide section
applicable to each catalogued failure mode. An operator reading a
Trust Report has a direct link from any recent failure event to the
recovery procedure.

## 5. Versioning

The recovery guide is versioned alongside the workspace. The v1.0
recovery guide pins to the v1.0 ABI; later patches update the guide
in lockstep. A guide-vs-implementation mismatch is treated as a fix
candidate per the v0.15-001 freeze discipline.

## 6. Acceptance criteria

1. `docs/operations/recovery-guide.md` exists and covers §2.
2. Every entry in v0.13-005's DR table (DR1–DR8) has a section in the
   guide.
3. Every boot/DTB failure mode from v0.12-003 has a section.
4. Every quarantine / rekey / partition scenario has a section.
5. All `bash run-verified` blocks pass `cargo xtask docs build`.
6. Follow-test attestation is committed.
7. Failure-mode catalogue is complete and matches the catalogued
   semantic intents.
8. Trust Report cross-references the guide sections.

## 7. Out of scope

- Multi-language operator guides. English only for v1.0.
- A separate "quick reference card" or "cheat sheet" — the guide's
  §2.1 triage page serves that purpose.
- Operator training materials beyond the guide.
- Vendor-specific deployment guidance (the deployment doc per target
  per RFC-v0.12-005 covers that).
- Recovery procedures for unsupported configurations.
