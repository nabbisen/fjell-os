# ADR-0005 — Semantic Stream First

**Status:** Accepted  
**Date:** 2026-05-04 (v0.1.0) / updated 2026-05-17 (v0.1.4, RFC 045)  
**Supersedes:** ADR-0004 Semantic Stream (renumbered)

---

## Context

Fjell OS needs an observable interface for operators and diagnostic tools.
Should the system expose a POSIX-style log file, a structured syslog, or
something designed for Fjell's specific use case?

---

## Decision

All observable system state is exported as a **semantic stream** — a
structured feed of StateNode, EventNode, and IntentNode values described
by `fjell-semantic-format`.

There is no raw log file. There is no syslog. The text proxy
(`fjell-proxy-text`) is a renderer of the semantic stream, not the primary
interface.

This approach is "semantic stream first": **design the semantic event
before implementing the behaviour** that produces it. An unobservable state
is treated as a bug.

---

## Consequences

- Every security-relevant event should appear in the semantic stream
  (see evidence-export audit, RFC 044).
- The stream is decoupled from any specific display format; a future GUI
  renderer, a JSON exporter, or a metrics endpoint can consume the same
  stream.
- The `fjell-proxy-text` renderer is a demonstration of the pattern, not
  the architecture.
- At v0.1.2, several events are missing from the stream (see RFC 044).
  v0.2 (RFC 041) closes the gaps.

---

## Security Boundary Impact

The semantic stream must surface security failures (capability denial,
revocation, quarantine, corruption). If failures are invisible, an operator
cannot investigate an incident.

The gap between "stream exists" and "failures appear in the stream" is the
evidence-export audit target (RFC 044) and the persistent evidence hardening
target (RFC 041, v0.2).

---

## Deferred Work

- All `✓` cells in the evidence matrix (RFC 044): v0.2, RFC 041.
- Stable semantic schema version: v0.5.
- Encrypted / authenticated stream export: no current milestone.

---

## Related RFCs

- RFC 013 (ADR Creation M6/M7)
- RFC 041 (Persistent Evidence Hardening, v0.2)
- RFC 044 (Audit/Snapshot/Semantic Evidence Audit, v0.1.3)
