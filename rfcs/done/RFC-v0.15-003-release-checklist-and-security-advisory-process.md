# RFC-v0.15-003 — Release Checklist and Security Advisory Process

**Status:** Implemented (v0.15.0)
**Target version:** v0.15.0
**Parent:** v0.15-001.
**Cross-refs:** RFC-v0.10-003 (reproducible build), v0.11-003 (signing),
    v0.13-003 (key compromise).

## 1. Problem

v1.0 is the first release that will be cited externally. Two
operational artefacts are missing:

- A release procedure that runs identically every time so the v1.0
  tag is not a one-off ceremony.
- A documented advisory process that defines what happens when a
  vulnerability is disclosed against Fjell.

Without these, the first incident or the first patch release becomes
an emergency. v0.15 lands both as committed, rehearsed procedures.

## 2. Release checklist

`docs/release/release-checklist.md` — a procedure that runs against a
clean checkout to produce signed v1.0 artefacts.

### 2.1 Sequence

```text
 1. Verify working copy is at the tagged commit; tree is clean.
 2. cargo xtask test-all                            (host + QEMU tiers)
 3. cargo xtask test-all --include-bench            (RFC-v0.10-004)
 4. cargo xtask repro-check                         (RFC-v0.10-003)
 5. cargo xtask trust-report                        (RFC 061 §6)
 6. cargo xtask docs build                          (RFC-v0.10-006)
 7. cargo xtask abi-snapshot --verify               (RFC-v0.10-002)
 8. Validate v1.0 readiness matrix has zero OPEN cells.
 9. cargo xtask release --version v1.0.0
       — produces release.tar.gz, release.txt
10. cargo xtask sign-bundle  (for each shipped bundle, RFC-v0.11-003)
11. Attest the release manifest with the v1.0 release key.
12. Publish release tarball + signatures to the release location.
13. Tag the commit v1.0.0.
14. Commit the Trust Report from step 5 to docs/release/v1.0.0/.
```

Each step has a documented expected output. The checklist is itself
verified by a tooling step:

```
cargo xtask release-checklist --dry-run
```

which walks the steps without producing artefacts and verifies that
each command is wired and the expected outputs match.

### 2.2 Reproducibility constraint

Steps 2, 4, 7, and 9 must produce bit-identical output on two
independent runs of the same checkout. The reproducible-build gate
(RFC-v0.10-003) enforces this.

### 2.3 Release-key handling

The v1.0 release key is distinct from the day-to-day signing key:

- Stored on a workstation that does **not** participate in CI.
- Used only at step 10–11 of the checklist.
- Rotated per the cadence in `docs/security/key-policy.md` (post-v1.0
  document; the v0.15 commitment is "rotation procedure exists from
  day one").

### 2.4 Rehearsal

The checklist must run end-to-end at least once *before* v1.0.0,
against a v0.15.0-rc candidate. The rehearsal exists to catch
process gaps; its output is discarded and the v1.0.0 release runs
the checklist cleanly afterwards.

## 3. Security advisory process

`docs/security/advisory-process.md` — what happens between
"vulnerability reported" and "patched release shipped."

### 3.1 Intake

A reporter contacts the project via a documented channel
(`security@<domain>` placeholder until set at landing). Intake
acknowledgement target: 72 hours.

### 3.2 Triage

Triage classifies severity using a small published rubric:

- **Critical.** Defeats I1–I8 directly; affects all deployed nodes;
  no operator-side mitigation.
- **High.** Defeats one invariant under realistic adversary capability;
  operator-side mitigation exists but is non-obvious.
- **Medium.** Weakens an invariant defence-in-depth; operator-side
  mitigation is straightforward.
- **Low.** Cosmetic, denial-of-service-only, or requires unrealistic
  adversary capability.

Each report receives a tracking identifier (FSAD-<year>-<seq>).

### 3.3 Disclosure timeline

- **Critical:** ≤ 30 days from triage to patch.
- **High:** ≤ 60 days.
- **Medium:** ≤ 90 days.
- **Low:** next regular release.

These are targets, not contractual commitments. Extension requires
written rationale in the advisory record.

### 3.4 Advisory artefact

For each closed advisory, a record committed to
`docs/security/advisories/FSAD-<year>-<seq>.md`:

```text
ID:            FSAD-2026-001
Severity:      High
Reported:      YYYY-MM-DD
Disclosed:     YYYY-MM-DD
Affected:      v0.x..v0.y
Fixed in:      v0.z
Reporter:      (name or "anonymous" with permission)
Description:   ...
Threat ref:    T13 (RFC-v0.15-002)
Mitigation:    ...
References:    fix commit hash, related RFCs
Reproducer:    available / withheld
```

### 3.5 CVE handling

If the issue warrants a CVE, the project requests one via the
appropriate CNA. The advisory record carries the CVE id once
assigned. CVE assignment does not delay the patched release; the
release ships when ready and the advisory is updated.

### 3.6 Coordinated disclosure

The default is coordinated disclosure: the reporter is asked to
withhold public discussion until the patch is available. The project
commits to the disclosure timeline in §3.3 as the reporter's outer
bound for compelling them to coordinate.

### 3.7 Rehearsal

Before v0.15 lands, the advisory process is exercised once
end-to-end against a synthetic report. The rehearsal output is
committed at `docs/security/advisory-process-rehearsal.md` as
evidence the procedure works.

## 4. Linkage with v0.13-003

A live advisory that is also a key compromise triggers the v0.13-003
playbook in parallel. The advisory process describes the
intake/disclosure; v0.13-003 describes the technical response. The
two are intentionally separate.

## 5. Acceptance criteria

1. `docs/release/release-checklist.md` exists and covers §2.
2. `cargo xtask release-checklist --dry-run` runs and validates each
   step.
3. A full rehearsal of the checklist is committed against a
   v0.15.0-rc candidate.
4. `docs/security/advisory-process.md` exists and covers §3.
5. Severity rubric, timelines, advisory artefact template, and CVE
   handling are all written.
6. The synthetic advisory rehearsal output is committed.
7. The v1.0 release key is defined and its handling is documented in
   `docs/security/key-policy.md` (placeholder content acceptable;
   filled at v1.0 landing).

## 6. Out of scope

- A bug-bounty programme.
- A public advisory database with structured machine-readable
  metadata beyond the per-advisory Markdown.
- Multi-party disclosure coordination (when third-party libraries are
  involved). Standard library-level disclosure handles these;
  Fjell-side procedure tracks them.
- Compliance-driven disclosure timelines (regulatory regimes). Out of
  scope for v1.0; operator handles their own regulatory obligations.
