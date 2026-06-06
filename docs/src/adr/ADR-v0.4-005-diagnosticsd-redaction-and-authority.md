# ADR-v0.4-005 — diagnosticsd: Typed Allow-List Redaction and Bundle Authority

**Status:** Accepted  
**Date:** 2026-05-19 (v0.4.0, RFC v0.4-005)

---

## Context

Remote observability of trust evidence requires sending diagnostic data off-
device.  Fjell has a strict privacy rule: no variable-length strings, no
payload bytes, no PII.  The question is: how do we ensure redaction is
correctly applied, and which service is authorised to create bundles?

## Decision

`diagnosticsd` is the only service with `ResourceClass::DiagBundle` authority.
cap-broker denies all other services this resource class explicitly (before
the default-deny catch-all).

The redaction rules are encoded structurally in `fjell-diag-format`'s
`BundleBuilder`, not in the source services:

- **Audit events**: only the 14 tags on `ALLOWED_AUDIT_KINDS` (§6.1) are
  admitted.  Any other `kind_tag` is silently dropped.
- **Semantic intents**: only the 9 tags on `ALLOWED_INTENT_TAGS` (§6.2) are
  admitted.
- **Fields**: only `seq`, `kind_tag`/`intent_tag`, `code`, `at_tick` are
  stored.  All variable-length fields (strings, paths, payload bytes) are
  structurally absent from `DiagAuditEvent` and `DiagIntent`.

The canonical bundle digest (`bundle_digest = SHA256("FJELL-DIAG-V1" || ...)`)
covers the schema version, bundle ID, timestamps, measurement head, last
attestation digest, and all admitted records.  The digest is computed in
`BundleBuilder::finalise()`.

Push is operator-initiated via `fjell-tools diag push`; no autonomous push.

## Consequences

- Redaction is testable: the 15 host unit tests in `fjell-diag-format` verify
  allow-list enforcement, capacity limits, and digest determinism.
- The `DiagBundle` resource class in cap-broker is an explicit, mandatory
  control point; `diagnosticsd` cannot be replaced by a rogue service without
  a cap-broker policy change (which requires a signed firmware update).
- The fixed-shape `DiagnosticBundle` struct (`MAX_AUDIT_EVENTS=64`,
  `MAX_SEMANTIC_INTENTS=32`) makes bundle size predictable and bounded.
- Attestation push (`AttestationPush`, RFC v0.4-005 §5.3) follows the same
  operator-initiated model, with `attestd` as the only producer of
  `SignedAttestationRecordV2` and `diagnosticsd` providing context records.
