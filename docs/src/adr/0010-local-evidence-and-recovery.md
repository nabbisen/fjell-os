# ADR-0010 — Local Evidence and Recovery

**Status:** Accepted  
**Date:** 2026-05-17 (v0.1.4, RFC 045) — captures decisions made in M7/M8  
**Supersedes:** ADR-0010 Inline Init Workaround (superseded by RFC 038)

---

## Context

Fjell OS is a security-oriented embedded system. When something goes wrong
— a store corruption, a signature failure, a missed health confirmation —
the system must be able to:

1. Record what happened (evidence).
2. Decide whether to continue or roll back (recovery).
3. Prevent the evidence from being silently dropped.

The related design question is: how much of this is "local" (no network,
no remote attestation) in v0.1.x vs. deferred?

---

## Decision

At v0.1.0, evidence collection and recovery are **entirely local**:

- The **measurement chain** is kernel-recorded and self-reported.
- The **local attestation record** is signed by a development-grade key
  held in the kernel binary.
- The **snapshot** captures system state at a point in time.
- Recovery decisions are made by `fjell-recoveryd` based on the snapshot
  and measurement chain — without any remote verifier.

This is not a production attestation scheme. It is the data shape that a
production scheme would use, verified locally.

---

## Consequences

- A compromised kernel can forge any measurement or attestation record.
  This is an explicit known limitation at v0.1.x.
- Local recovery (rollback, re-verify) works correctly within the trust
  model of a single, uncompromised boot.
- The evidence data shapes (`AuditRecord`, `SnapshotRecord`,
  `MeasurementEvent`, `AttestationRecord`) are ABI-stable enough to be
  repurposed when hardware-rooted trust arrives (v0.3).

---

## Security Boundary Impact

Evidence must surface security failures. At v0.1.2, many failure events
are missing from the evidence channels (see RFC 044 matrix). v0.2 (RFC 041)
closes the gap.

The measurement chain cannot be trusted if the kernel is compromised.
This limits the security claim to: "this system has not been compromised
above the kernel layer." Hardware-rooted trust (v0.3) extends the claim
to: "this firmware and kernel have not been tampered with."

---

## Deferred Work

- Hardware-rooted trust for measurement (v0.3).
- Remote attestation protocol (v0.4).
- Encrypted evidence export (no current milestone).
- Evidence visibility gaps: v0.2, RFC 041.

---

## Related RFCs

- RFC 020 (Audit Drain, M4), RFC 021 (Cap-Broker Policy Evaluation, M4)
- RFC 023 (BCB Mirror Selection Tests)
- RFC 039 (Safe User Copy + Real Audit Drain, v0.2)
- RFC 041 (Persistent Evidence Hardening, v0.2)
- RFC 044 (Evidence Export Audit, v0.1.3)
